use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use assert_fs::fixture::{ChildPath, PathChild};
use assert_fs::prelude::*;
use fs2::FileExt;
use predicates::prelude::*;
use serde_json::{json, Value};
use t3_tape::patch::{surface_hash, UnifiedDiff};

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

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
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

fn plugin_root(temp: &assert_fs::TempDir) -> ChildPath {
    temp.child(".t3/patch")
}

#[test]
fn patch_add_fails_when_state_lock_held() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    let lock_path = plugin_root(&temp).child("state.lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(lock_path.path())
        .unwrap();
    file.lock_exclusive().unwrap();

    tracked.write_str("alpha\npatched\n").unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["patch", "add", "--title", "x", "--intent", "y"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("state lock held"));
}

fn set_pre_patch_hook(temp: &assert_fs::TempDir, command: &str) {
    let config_path = plugin_root(temp).child("config.json");
    let mut config: Value =
        serde_json::from_str(&fs::read_to_string(config_path.path()).unwrap()).unwrap();
    config["hooks"]["pre-patch"] = json!(command);
    fs::write(
        config_path.path(),
        serde_json::to_string_pretty(&config).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn patch_id_allocation_increments_correctly() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nbeta\nfirst change\n").unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "first-change",
            "--intent",
            "Record the first tracked change.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-001"));

    commit_all(&temp, "first patch baseline");

    tracked
        .write_str("alpha\nbeta\nfirst change\nsecond change\n")
        .unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "second-change",
            "--intent",
            "Record the second tracked change.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-002"));

    assert!(temp
        .child(".t3/patch/patches/PATCH-001.diff")
        .path()
        .is_file());
    assert!(temp
        .child(".t3/patch/patches/PATCH-002.diff")
        .path()
        .is_file());

    let patch_md = fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap();
    let first_index = patch_md.find("## [PATCH-001] first-change").unwrap();
    let second_index = patch_md.find("## [PATCH-002] second-change").unwrap();
    assert!(
        first_index < second_index,
        "patch registry ordering drifted"
    );
}

#[test]
fn patch_add_writes_diff_meta_and_appends_patch_md() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nbeta updated\n").unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "toolbar-button",
            "--intent",
            "Add a toolbar button to the tracked file.",
            "--assert",
            "toolbar button renders in the tracked file",
            "--assert",
            "toolbar button remains editable after refresh",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("created patch PATCH-001"));

    let diff_path = temp.child(".t3/patch/patches/PATCH-001.diff");
    let meta_path = temp.child(".t3/patch/patches/PATCH-001.meta.json");
    assert!(diff_path.path().is_file());
    assert!(meta_path.path().is_file());

    let meta: Value = serde_json::from_str(&fs::read_to_string(meta_path.path()).unwrap()).unwrap();
    assert_eq!(meta["id"], "PATCH-001");
    assert_eq!(meta["title"], "toolbar-button");
    assert_eq!(meta["status"], "active");
    assert_eq!(
        meta["behavior-assertions"][0],
        "toolbar button renders in the tracked file"
    );

    let patch_md = fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap();
    assert!(patch_md.contains("## [PATCH-001] toolbar-button"));
    assert!(patch_md.contains("### Intent"));
    assert!(patch_md.contains("### Behavior Contract"));
    assert!(patch_md.contains("### Scope"));
    assert!(patch_md.contains("### Dependencies"));
}

#[test]
fn patch_add_is_atomic_when_patch_md_write_fails() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nforced failure\n").unwrap();
    let patch_md_before = fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .env("T3_TAPE_INTERNAL_TEST_FAIL_AFTER_PATCH_FILES", "1")
        .args([
            "patch",
            "add",
            "--title",
            "atomic-check",
            "--intent",
            "Exercise rollback.",
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("injected failure"));

    assert!(!temp
        .child(".t3/patch/patches/PATCH-001.diff")
        .path()
        .exists());
    assert!(!temp
        .child(".t3/patch/patches/PATCH-001.meta.json")
        .path()
        .exists());
    assert_eq!(
        fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap(),
        patch_md_before
    );
}

#[test]
fn patch_list_and_show_parse_recorded_patch() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nlist and show\n").unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "list-show",
            "--intent",
            "Create a record for list/show coverage.",
        ])
        .assert()
        .success();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["patch", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-001\tlist-show\tactive"));

    t3_tape_command()
        .current_dir(temp.path())
        .args(["patch", "show", "PATCH-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("## [PATCH-001] list-show"))
        .stdout(predicate::str::contains("meta:"));
}

#[test]
fn patch_show_diff_prints_expected_path() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nshow diff path\n").unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "diff-path",
            "--intent",
            "Create a diff path record.",
        ])
        .assert()
        .success();

    t3_tape_command()
        .current_dir(temp.path())
        .args(["patch", "show", "PATCH-001", "--diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains(".t3"))
        .stdout(predicate::str::contains("PATCH-001.diff"));
}

#[test]
fn surface_hash_is_stable_for_fixed_diff_fixture() {
    let diff = include_str!("fixtures/multi-file.diff");
    let parsed = UnifiedDiff::parse(diff).unwrap();
    let hash = surface_hash::compute(&parsed);
    assert_eq!(
        hash,
        "bb685fb5bd74a491b75cb5ec3e3dface82e58da389e56d22f24db1c8b65e4681"
    );
}

#[test]
fn patch_import_clustering_creates_multiple_deterministic_patch_records() {
    let temp = assert_fs::TempDir::new().unwrap();
    seed_repo(&temp);
    run_init(&temp);

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "import",
            "--diff",
            fixture_path("multi-file.diff").to_str().unwrap(),
        ])
        .write_stdin(
            "y\nalpha-import\nDescribe the alpha import.\nbeta-import\nDescribe the beta import.\n",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("created patch PATCH-001"))
        .stdout(predicate::str::contains("created patch PATCH-002"));

    let patch_md = fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap();
    assert!(patch_md.contains("## [PATCH-001] alpha-import"));
    assert!(patch_md.contains("## [PATCH-002] beta-import"));
}

#[test]
fn pre_patch_hook_failure_aborts_without_partial_state() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);
    set_pre_patch_hook(&temp, "exit /b 9");

    tracked.write_str("alpha\nhook failure\n").unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "hook-failure",
            "--intent",
            "Abort before writing patch state.",
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("hook failed"));

    assert!(!temp
        .child(".t3/patch/patches/PATCH-001.diff")
        .path()
        .exists());
    assert!(!temp
        .child(".t3/patch/patches/PATCH-001.meta.json")
        .path()
        .exists());
    let patch_md = fs::read_to_string(temp.child(".t3/patch.md").path()).unwrap();
    assert!(!patch_md.contains("PATCH-001"));
}

#[test]
fn export_writes_compact_markdown_summary() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\nexport coverage\n").unwrap();
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "exportable",
            "--intent",
            "Create a patch that can be exported.",
            "--assert",
            "export summary includes this assertion",
        ])
        .assert()
        .success();

    let output = temp.child("CUSTOMIZATIONS.md");
    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "export",
            "--format",
            "markdown",
            "--output",
            output.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let rendered = fs::read_to_string(output.path()).unwrap();
    assert!(rendered.contains("# PatchMD Export"));
    assert!(rendered.contains("## [PATCH-001] exportable"));
    assert!(rendered.contains("Create a patch that can be exported."));
    assert!(rendered.contains("export summary includes this assertion"));
}

#[test]
fn patch_add_excludes_patchmd_owned_state_from_recorded_diff() {
    let temp = assert_fs::TempDir::new().unwrap();
    let tracked = seed_repo(&temp);
    run_init(&temp);

    tracked.write_str("alpha\napp change\n").unwrap();
    temp.child(".t3/patch/migration.log")
        .write_str("owned-state-only change\n")
        .unwrap();
    temp.child(".t3/patch/migration.log")
        .write_str("temporary owned-state noise\n")
        .unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "exclude-owned-state",
            "--intent",
            "Record only app code changes.",
        ])
        .assert()
        .success();

    let diff = fs::read_to_string(temp.child(".t3/patch/patches/PATCH-001.diff").path()).unwrap();
    assert!(diff.contains("src/app.txt"));
    assert!(!diff.contains(".t3/patch.md"));
    assert!(!diff.contains(".t3/patch/migration.log"));
}

#[test]
fn patch_add_fails_when_remaining_diff_only_contains_patchmd_owned_state() {
    let temp = assert_fs::TempDir::new().unwrap();
    seed_repo(&temp);
    run_init(&temp);
    commit_all(&temp, "track patchmd state");

    temp.child(".t3/patch/migration.log")
        .write_str("owned-state-only change\n")
        .unwrap();

    t3_tape_command()
        .current_dir(temp.path())
        .args([
            "patch",
            "add",
            "--title",
            "state-only",
            "--intent",
            "Should fail when only PatchMD state changed.",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("remaining changes only touch PatchMD-owned state"));
}
