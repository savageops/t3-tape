use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

fn t3_tape_command() -> Command {
    Command::cargo_bin("t3-tape").expect("t3-tape binary should build")
}

fn git_init(path: &Path) {
    let status = Command::new("git")
        .arg("init")
        .current_dir(path)
        .status()
        .expect("git init should run");
    assert!(status.success(), "git init should succeed");

    for args in [
        ["config", "user.name", "Test User"],
        ["config", "user.email", "test@example.com"],
        ["config", "core.autocrlf", "false"],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .status()
            .expect("git config should run");
        assert!(status.success(), "git config should succeed");
    }

    fs::write(path.join("README.md"), "baseline\n").unwrap();
    let add_status = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .status()
        .expect("git add should run");
    assert!(add_status.success(), "git add should succeed");
    let commit_status = Command::new("git")
        .args(["commit", "-m", "baseline", "--quiet"])
        .current_dir(path)
        .status()
        .expect("git commit should run");
    assert!(commit_status.success(), "git commit should succeed");
}

fn git_head(path: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .expect("git rev-parse should run");
    assert!(output.status.success(), "git rev-parse should succeed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn canonical_recovery_restores_entrypoints() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for file in [
        src_root.join("main.rs"),
        src_root.join("lib.rs"),
        src_root.join("exit.rs"),
        src_root.join("cli").join("mod.rs"),
    ] {
        assert!(
            file.is_file(),
            "expected canonical file: {}",
            file.display()
        );
    }

    for stale in [
        src_root.join("main.rs.bak"),
        src_root.join("lib.rs.bak"),
        src_root.join("exit.rs.bak"),
        src_root.join("cli").join("mod.rs.bak"),
        src_root.join("store").join("zzz.rs"),
    ] {
        assert!(
            !stale.exists(),
            "stale recovery artifact should be removed: {}",
            stale.display()
        );
    }
}

#[test]
fn init_creates_expected_tree() {
    let temp = assert_fs::TempDir::new().unwrap();
    git_init(temp.path());
    temp.child("nested/deeper").create_dir_all().unwrap();

    t3_tape_command()
        .current_dir(temp.child("nested/deeper").path())
        .args([
            "init",
            "--upstream",
            "https://example.com/org/upstream.git",
            "--base-ref",
            "HEAD",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(".t3"));

    let state_dir = temp.child(".t3");
    assert!(state_dir.path().is_dir());
    assert!(state_dir.child("patches").path().is_dir());
    assert!(state_dir.child("sandbox").path().is_dir());
    assert!(state_dir.child("config.json").path().is_file());
    assert!(state_dir.child("patch.md").path().is_file());
    assert!(state_dir.child("migration.log").path().is_file());
    assert!(state_dir.child("triage.json").path().is_file());

    let config = fs::read_to_string(state_dir.child("config.json").path()).unwrap();
    assert!(config.contains("\"protocol\": \"0.1.0\""));
    assert!(config.contains("\"upstream\": \"https://example.com/org/upstream.git\""));

    let patch_md = fs::read_to_string(state_dir.child("patch.md").path()).unwrap();
    assert!(patch_md.contains("# PatchMD"));
    assert!(patch_md.contains("> project: upstream"));
    assert!(patch_md.contains(&format!("> base-ref: {}", git_head(temp.path()))));

    let triage = fs::read_to_string(state_dir.child("triage.json").path()).unwrap();
    assert_eq!(triage, "{}\n");
}

#[test]
fn init_is_idempotent() {
    let temp = assert_fs::TempDir::new().unwrap();
    git_init(temp.path());

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "init",
            "--upstream",
            "https://example.com/org/upstream.git",
            "--base-ref",
            "HEAD",
        ])
        .assert()
        .success();

    let state_dir = temp.child(".t3");
    let config_before = fs::read_to_string(state_dir.child("config.json").path()).unwrap();
    let patch_before = fs::read_to_string(state_dir.child("patch.md").path()).unwrap();
    let triage_before = fs::read_to_string(state_dir.child("triage.json").path()).unwrap();
    let migration_len_before = fs::metadata(state_dir.child("migration.log").path())
        .unwrap()
        .len();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "init",
            "--upstream",
            "https://example.com/other/project.git",
            "--base-ref",
            "v2",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(state_dir.child("config.json").path()).unwrap(),
        config_before
    );
    assert_eq!(
        fs::read_to_string(state_dir.child("patch.md").path()).unwrap(),
        patch_before
    );
    assert_eq!(
        fs::read_to_string(state_dir.child("triage.json").path()).unwrap(),
        triage_before
    );
    assert_eq!(
        fs::metadata(state_dir.child("migration.log").path())
            .unwrap()
            .len(),
        migration_len_before
    );
}

#[test]
fn init_respects_repo_root_override() {
    let temp = assert_fs::TempDir::new().unwrap();
    let project_root = temp.child("project-root");
    let runner = temp.child("runner");
    project_root.create_dir_all().unwrap();
    runner.create_dir_all().unwrap();
    git_init(project_root.path());

    t3_tape_command()
        .current_dir(runner.path())
        .arg("init")
        .arg("--upstream")
        .arg("https://example.com/org/upstream.git")
        .arg("--base-ref")
        .arg("HEAD")
        .arg("--repo-root")
        .arg(project_root.path())
        .assert()
        .success();

    assert!(project_root.child(".t3").path().is_dir());
    assert!(!runner.child(".t3").path().exists());
}

#[test]
fn init_respects_state_dir_override() {
    let temp = assert_fs::TempDir::new().unwrap();
    git_init(temp.path());

    t3_tape_command()
        .current_dir(temp.path())
        .arg("init")
        .arg("--upstream")
        .arg("https://example.com/org/upstream.git")
        .arg("--base-ref")
        .arg("HEAD")
        .arg("--state-dir")
        .arg(".t3-tape-state")
        .assert()
        .success()
        .stdout(predicate::str::contains(".t3-tape-state"));

    assert!(temp.child(".t3-tape-state").path().is_dir());
    assert!(!temp.child(".t3").path().exists());
}

#[test]
fn init_refuses_to_overwrite_non_empty_files() {
    let temp = assert_fs::TempDir::new().unwrap();
    let state_dir = temp.child(".t3");
    git_init(temp.path());
    state_dir.create_dir_all().unwrap();
    state_dir.child("patches").create_dir_all().unwrap();
    state_dir.child("sandbox").create_dir_all().unwrap();
    state_dir
        .child("config.json")
        .write_str("custom-config\n")
        .unwrap();
    state_dir
        .child("patch.md")
        .write_str("custom-patch\n")
        .unwrap();
    state_dir
        .child("migration.log")
        .write_str("custom-log\n")
        .unwrap();
    state_dir
        .child("triage.json")
        .write_str("custom-triage\n")
        .unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "init",
            "--upstream",
            "https://example.com/org/upstream.git",
            "--base-ref",
            "HEAD",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(state_dir.child("config.json").path()).unwrap(),
        "custom-config\n"
    );
    assert_eq!(
        fs::read_to_string(state_dir.child("patch.md").path()).unwrap(),
        "custom-patch\n"
    );
    assert_eq!(
        fs::read_to_string(state_dir.child("migration.log").path()).unwrap(),
        "custom-log\n"
    );
    assert_eq!(
        fs::read_to_string(state_dir.child("triage.json").path()).unwrap(),
        "custom-triage\n"
    );
}

#[test]
fn init_tolerates_foreign_reports_directory() {
    let temp = assert_fs::TempDir::new().unwrap();
    git_init(temp.path());

    let reports_dir = temp.child(".t3/reports");
    reports_dir.create_dir_all().unwrap();
    let report_file = reports_dir.child("existing-report.md");
    report_file.write_str("keep me\n").unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "init",
            "--upstream",
            "https://example.com/org/upstream.git",
            "--base-ref",
            "HEAD",
        ])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(report_file.path()).unwrap(), "keep me\n");
    assert!(temp.child(".t3/config.json").path().is_file());
    assert!(temp.child(".t3/patches").path().is_dir());
    assert!(temp.child(".t3/sandbox").path().is_dir());
}
