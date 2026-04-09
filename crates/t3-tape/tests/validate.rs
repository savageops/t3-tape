use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use assert_fs::fixture::{ChildPath, PathChild};
use assert_fs::prelude::*;
use predicates::prelude::*;
use serde_json::Value;

fn t3_tape_command() -> Command {
    Command::cargo_bin("t3-tape").expect("t3-tape binary should build")
}

fn git(repo_root: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .args(args)
        .current_dir(repo_root)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git command failed: {:?}", args);
}

fn seed_repo(temp: &assert_fs::TempDir) -> ChildPath {
    git(temp.path(), &["init"]);
    git(temp.path(), &["config", "user.name", "Test User"]);
    git(temp.path(), &["config", "user.email", "test@example.com"]);
    git(temp.path(), &["config", "core.autocrlf", "false"]);

    let tracked = temp.child("src/app.txt");
    tracked.write_str("alpha\nbeta\n").unwrap();
    git(temp.path(), &["add", "."]);
    git(temp.path(), &["commit", "-m", "baseline", "--quiet"]);
    tracked
}

fn commit_all(temp: &assert_fs::TempDir, message: &str) {
    git(temp.path(), &["add", "."]);
    git(temp.path(), &["commit", "-m", message, "--quiet"]);
}

fn run_init(temp: &assert_fs::TempDir) {
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
}

fn add_patch(temp: &assert_fs::TempDir, tracked: &ChildPath, title: &str, intent: &str) {
    tracked.write_str(&format!("alpha\n{title}\n")).unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args(["patch", "add", "--title", title, "--intent", intent])
        .assert()
        .success();
}

fn patch_md_path(temp: &assert_fs::TempDir) -> PathBuf {
    temp.child(".t3/patch.md").path().to_path_buf()
}

fn meta_path(temp: &assert_fs::TempDir, id: &str) -> PathBuf {
    temp.child(format!(".t3/patches/{id}.meta.json"))
        .path()
        .to_path_buf()
}

fn diff_path(temp: &assert_fs::TempDir, id: &str) -> PathBuf {
    temp.child(format!(".t3/patches/{id}.diff"))
        .path()
        .to_path_buf()
}

#[test]
fn validate_passes_with_reports_directory_and_placeholder_triage() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "validate-pass",
        "Create a valid patch record.",
    );

    temp.child(".t3/reports").create_dir_all().unwrap();
    temp.child(".t3/reports/example-summary.md")
        .write_str("keep me\n")
        .unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::eq("OK\n"));
}

#[test]
fn validate_fails_when_diff_is_missing() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(&temp, &tracked, "missing-diff", "Exercise diff validation.");

    fs::remove_file(diff_path(&temp, "PATCH-001")).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("missing diff file"));
}

#[test]
fn validate_fails_when_meta_is_missing() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(&temp, &tracked, "missing-meta", "Exercise meta validation.");

    fs::remove_file(meta_path(&temp, "PATCH-001")).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("missing meta file"));
}

#[test]
fn validate_fails_when_meta_fields_drift() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "meta-drift",
        "Exercise meta drift validation.",
    );

    let path = meta_path(&temp, "PATCH-001");
    let mut meta: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    meta["title"] = Value::String("different-title".to_string());
    fs::write(&path, serde_json::to_string_pretty(&meta).unwrap() + "\n").unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("meta title mismatch"));
}

#[test]
fn validate_fails_when_status_is_unknown() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "unknown-status",
        "Exercise status validation.",
    );

    let patch_md = patch_md_path(&temp);
    let current = fs::read_to_string(&patch_md).unwrap();
    let updated = current.replacen("**status:** active", "**status:** sideways", 1);
    fs::write(&patch_md, updated).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("unsupported status `sideways`"));
}

#[test]
fn validate_fails_on_dependency_cycle() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    add_patch(&temp, &tracked, "first", "First patch.");
    commit_all(&temp, "first patch state");
    tracked.write_str("alpha\nfirst\nsecond\n").unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "second",
            "--intent",
            "Second patch.",
        ])
        .assert()
        .success();

    let patch_md = patch_md_path(&temp);
    let current = fs::read_to_string(&patch_md).unwrap();
    let first = current.replacen("- **requires:** []", "- **requires:** [PATCH-002]", 1);
    let second = first.replacen("- **requires:** []", "- **requires:** [PATCH-001]", 1);
    fs::write(&patch_md, second).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("dependency cycle detected"));
}

#[test]
fn validate_accepts_missing_or_placeholder_triage_summary() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "triage-placeholder",
        "Exercise triage placeholder validation.",
    );

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .success();

    fs::remove_file(temp.child(".t3/triage.json").path()).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .success();
}

#[test]
fn validate_rejects_invalid_triage_schema() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "triage-invalid",
        "Exercise triage schema validation.",
    );

    temp.child(".t3/triage.json")
        .write_str("{\"schema-version\":\"9.9.9\"}\n")
        .unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains(
            "triage summary schema-version must be 0.1.0",
        ));
}

#[test]
fn validate_staged_fails_without_matching_patchmd_updates() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nstaged only\n").unwrap();
    git(temp.path(), &["add", "src/app.txt"]);

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate", "--staged"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains(
            "staged project code changes require PatchMD updates",
        ))
        .stderr(predicate::str::is_empty());
}

#[test]
fn validate_staged_passes_with_two_layer_changes_staged() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "staged-pass",
        "Create a staged patch record.",
    );

    git(
        temp.path(),
        &[
            "add",
            "src/app.txt",
            ".t3/patch.md",
            ".t3/patches/PATCH-001.diff",
            ".t3/patches/PATCH-001.meta.json",
        ],
    );

    t3_tape_command()
        .current_dir(temp.path())
        .args(["validate", "--staged"])
        .assert()
        .success()
        .stdout(predicate::eq("OK\n"));
}

#[test]
fn validate_json_outputs_stable_schema_for_success_and_failure() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    add_patch(
        &temp,
        &tracked,
        "json-report",
        "Create a JSON report patch.",
    );

    let success = t3_tape_command()
        .current_dir(temp.path())
        .args(["--json", "validate"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let success_report: Value = serde_json::from_slice(&success).unwrap();
    assert_eq!(success_report["schema-version"], "0.1.0");
    assert_eq!(success_report["status"], "ok");
    assert_eq!(success_report["errors"].as_array().unwrap().len(), 0);
    assert_eq!(success_report["warnings"].as_array().unwrap().len(), 0);

    fs::remove_file(meta_path(&temp, "PATCH-001")).unwrap();

    let failure = t3_tape_command()
        .current_dir(temp.path())
        .args(["--json", "validate"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let failure_report: Value = serde_json::from_slice(&failure).unwrap();
    assert_eq!(failure_report["schema-version"], "0.1.0");
    assert_eq!(failure_report["status"], "error");
    assert!(failure_report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry.as_str().unwrap().contains("missing meta file")));
}

#[test]
fn hooks_print_outputs_are_stable() {
    t3_tape_command()
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .args(["hooks", "print", "pre-commit"])
        .assert()
        .success()
        .stdout(predicate::eq(
            "#!/bin/sh\nt3-tape validate --staged\nif [ $? -ne 0 ]; then\n  echo \"PatchMD: staged changes missing intent entry. Run: t3-tape patch add\"\n  exit 1\nfi\n",
        ));

    t3_tape_command()
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .args(["hooks", "print", "gitignore"])
        .assert()
        .success()
        .stdout(predicate::eq(
            ".t3/sandbox/\n.t3/config.json.local\n",
        ));

    t3_tape_command()
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .args(["hooks", "print", "gitattributes"])
        .assert()
        .success()
        .stdout(predicate::eq(
            ".t3/patch.md merge=union\n.t3/migration.log merge=union\n",
        ));
}

#[test]
fn hooks_install_refuses_overwrite_without_force() {
    let temp = assert_fs::TempDir::new().unwrap();
    seed_repo(&temp);
    run_init(&temp);

    t3_tape_command()
        .current_dir(temp.path())
        .args(["hooks", "install", "pre-commit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("installed pre-commit hook"));

    t3_tape_command()
        .current_dir(temp.path())
        .args(["hooks", "install", "pre-commit"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "refusing to overwrite existing pre-commit hook",
        ));

    t3_tape_command()
        .current_dir(temp.path())
        .args(["hooks", "install", "pre-commit", "--force"])
        .assert()
        .success();
}
