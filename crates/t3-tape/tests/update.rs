use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use assert_fs::fixture::{FileWriteStr, PathChild, PathCreateDir};
use assert_fs::TempDir;
use fs2::FileExt;
use predicates::prelude::*;
use serde_json::{json, Value};
use t3_tape::agent;
use t3_tape::agent::schema::{ConflictResolutionRequest, ConflictResolutionResponse};
use t3_tape::patch::{surface_hash, UnifiedDiff};
use t3_tape::store::schema::AgentConfig;

fn t3_tape_command() -> Command {
    Command::cargo_bin("t3-tape").expect("t3-tape binary should build")
}

fn git(repo_root: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git command failed: {:?}\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(repo_root: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git command failed: {:?}\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn configure_git_identity(repo_root: &Path) {
    git(repo_root, &["config", "user.name", "Test User"]);
    git(repo_root, &["config", "user.email", "test@example.com"]);
    git(repo_root, &["config", "core.autocrlf", "false"]);
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn update_fails_when_state_lock_held() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);

    let lock_path = pair.fork.join(".t3/patch/state.lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .unwrap();
    file.lock_exclusive().unwrap();

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", "HEAD"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("state lock held"));
}

struct RepoPair {
    temp: TempDir,
    upstream: PathBuf,
    fork: PathBuf,
}

impl RepoPair {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let upstream = temp.child("upstream");
        upstream.create_dir_all().unwrap();
        git(upstream.path(), &["init"]);
        configure_git_identity(upstream.path());
        write_file(&upstream.path().join("src/app.txt"), "alpha\nbase\n");
        git(upstream.path(), &["add", "."]);
        git(upstream.path(), &["commit", "-m", "baseline", "--quiet"]);

        let fork = temp.child("fork");
        let output = StdCommand::new("git")
            .arg("clone")
            .arg(upstream.path())
            .arg(fork.path())
            .output()
            .expect("git clone should run");
        assert!(
            output.status.success(),
            "git clone failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        configure_git_identity(fork.path());

        Self {
            temp,
            upstream: upstream.path().to_path_buf(),
            fork: fork.path().to_path_buf(),
        }
    }
}

fn run_init(fork_root: &Path, upstream_root: &Path) {
    t3_tape_command()
        .current_dir(fork_root)
        .args([
            "init",
            "--upstream",
            upstream_root.to_str().unwrap(),
            "--base-ref",
            "HEAD",
        ])
        .assert()
        .success();
}

fn add_patch_and_commit(
    fork_root: &Path,
    relative_path: &str,
    content: &str,
    title: &str,
    intent: &str,
) {
    write_file(&fork_root.join(relative_path), content);
    t3_tape_command()
        .current_dir(fork_root)
        .args(["patch", "add", "--title", title, "--intent", intent])
        .assert()
        .success();
    git(fork_root, &["add", "."]);
    git(fork_root, &["commit", "-m", title, "--quiet"]);
}

fn commit_change(repo_root: &Path, relative_path: &str, content: &str, message: &str) -> String {
    write_file(&repo_root.join(relative_path), content);
    git(repo_root, &["add", "."]);
    git(repo_root, &["commit", "-m", message, "--quiet"]);
    git_output(repo_root, &["rev-parse", "HEAD"])
}

fn delete_and_commit(repo_root: &Path, relative_path: &str, message: &str) -> String {
    fs::remove_file(repo_root.join(relative_path)).unwrap();
    git(repo_root, &["add", "-A"]);
    git(repo_root, &["commit", "-m", message, "--quiet"]);
    git_output(repo_root, &["rev-parse", "HEAD"])
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_config_value(fork_root: &Path, key_path: &[&str], value: Value) {
    let config_path = fork_root.join(".t3/patch/config.json");
    let mut config: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    let mut current = &mut config;
    for key in &key_path[..key_path.len() - 1] {
        current = current.get_mut(*key).unwrap();
    }
    current[key_path[key_path.len() - 1]] = value;
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).unwrap() + "\n",
    )
    .unwrap();
}

fn configure_exec_agent(fork_root: &Path, endpoint: &str, threshold: f64) {
    write_config_value(fork_root, &["agent", "provider"], json!("exec"));
    write_config_value(fork_root, &["agent", "endpoint"], json!(endpoint));
    write_config_value(
        fork_root,
        &["agent", "confidence-threshold"],
        json!(threshold),
    );
}

fn configure_preview_command(fork_root: &Path, command: &str) {
    write_config_value(fork_root, &["sandbox", "preview-command"], json!(command));
}

fn set_patch_requires(fork_root: &Path, patch_id: &str, requires: &[&str]) {
    let patch_md_path = fork_root.join(".t3/patch.md");
    let current = fs::read_to_string(&patch_md_path).unwrap();
    let marker = format!("## [{patch_id}]");
    let start = current.find(&marker).unwrap();
    let end = current[start + marker.len()..]
        .find("\n## [")
        .map(|offset| start + marker.len() + offset)
        .unwrap_or(current.len());
    let block = &current[start..end];
    let updated_block = block.replacen(
        "- **requires:** []",
        &format!("- **requires:** [{}]", requires.join(", ")),
        1,
    );
    let updated = format!("{}{}{}", &current[..start], updated_block, &current[end..]);
    fs::write(&patch_md_path, updated).unwrap();
}

fn patch_md_base_ref(fork_root: &Path) -> String {
    fs::read_to_string(fork_root.join(".t3/patch.md"))
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("> base-ref: ").map(str::to_string))
        .unwrap()
}

fn latest_sandbox_dir(fork_root: &Path) -> PathBuf {
    let mut dirs = fs::read_dir(fork_root.join(".t3/patch/sandbox"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    dirs.sort();
    assert_eq!(dirs.len(), 1, "expected exactly one sandbox directory");
    dirs.remove(0)
}

fn write_exec_agent(temp: &TempDir, name: &str, response: &Value) -> String {
    let response_path = temp.child(format!("{name}-response.json"));
    response_path
        .write_str(&(serde_json::to_string_pretty(response).unwrap() + "\n"))
        .unwrap();

    if cfg!(windows) {
        let script = temp.child(format!("{name}.cmd"));
        script
            .write_str(&format!(
                "@echo off\r\ntype \"{}\"\r\n",
                response_path.path().display()
            ))
            .unwrap();
        script.path().display().to_string()
    } else {
        let script = temp.child(format!("{name}.sh"));
        script
            .write_str(&format!(
                "#!/bin/sh\ncat \"{}\"\n",
                response_path.path().display()
            ))
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(script.path()).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script.path(), perms).unwrap();
        }
        script.path().display().to_string()
    }
}

fn conflict_resolution_diff() -> String {
    "diff --git a/src/app.txt b/src/app.txt\n--- a/src/app.txt\n+++ b/src/app.txt\n@@ -1,2 +1,2 @@\n alpha\n-upstream\n+patched\n".to_string()
}

fn rederive_diff() -> String {
    "diff --git a/src/app.txt b/src/app.txt\nnew file mode 100644\n--- /dev/null\n+++ b/src/app.txt\n@@ -0,0 +1,2 @@\n+alpha\n+patched\n".to_string()
}

fn failing_preview_command() -> &'static str {
    if cfg!(windows) {
        "exit /b 7"
    } else {
        "exit 7"
    }
}

#[test]
fn update_clean_triage_persists_artifacts_and_preserves_head() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\nbase\n",
        "clean-patch",
        "Keep the fork patch active across an unrelated upstream change.",
    );

    let head_before = git_output(&pair.fork, &["rev-parse", "HEAD"]);
    let to_ref = commit_change(&pair.upstream, "README.md", "# upstream\n", "add readme");

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .success()
        .stdout(predicate::str::contains("CLEAN"));

    assert_eq!(git_output(&pair.fork, &["rev-parse", "HEAD"]), head_before);

    let latest_triage_path = pair.fork.join(".t3/patch/triage.json");
    let persisted = read_json(&latest_triage_path);
    let sandbox_triage = read_json(&latest_sandbox_dir(&pair.fork).join("triage.json"));
    assert_eq!(persisted, sandbox_triage);
    assert_eq!(persisted["patches"][0]["detected-status"], "CLEAN");
    assert_eq!(persisted["patches"][0]["triage-status"], "CLEAN");

    let triage_output = t3_tape_command()
        .current_dir(&pair.fork)
        .args(["--json", "triage"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let triage_json: Value = serde_json::from_slice(&triage_output).unwrap();
    assert_eq!(triage_json, persisted);

    let migration_log = fs::read_to_string(pair.fork.join(".t3/patch/migration.log")).unwrap();
    assert!(migration_log.contains("STARTED"));
    assert!(migration_log.contains("TRIAGED"));
}

#[test]
fn update_ci_exits_three_on_conflict_and_persists_triage() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\n",
        "conflict-patch",
        "Keep the forked line change even when upstream rewrites it.",
    );

    let head_before = git_output(&pair.fork, &["rev-parse", "HEAD"]);
    let to_ref = commit_change(
        &pair.upstream,
        "src/app.txt",
        "alpha\nupstream\n",
        "upstream conflict",
    );

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref, "--ci"])
        .assert()
        .failure()
        .code(3)
        .stdout(predicate::str::contains("NEEDS-YOU"));

    assert_eq!(git_output(&pair.fork, &["rev-parse", "HEAD"]), head_before);

    let persisted = read_json(&pair.fork.join(".t3/patch/triage.json"));
    assert_eq!(persisted["patches"][0]["detected-status"], "CONFLICT");
    assert_eq!(persisted["patches"][0]["triage-status"], "NEEDS-YOU");

    let triage_output = t3_tape_command()
        .current_dir(&pair.fork)
        .args(["--json", "triage"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let triage_json: Value = serde_json::from_slice(&triage_output).unwrap();
    assert_eq!(triage_json, persisted);
}

#[test]
fn agent_provider_contracts_are_stable() {
    let temp = TempDir::new().unwrap();
    let endpoint = write_exec_agent(
        &temp,
        "provider-stub",
        &json!({
            "resolved-diff": "diff --git a/src/app.txt b/src/app.txt\n--- a/src/app.txt\n+++ b/src/app.txt\n",
            "confidence": 0.91,
            "notes": "Stable stub response.",
            "unresolved": [],
        }),
    );

    let config = AgentConfig {
        provider: "exec".to_string(),
        endpoint,
        confidence_threshold: 0.80,
        max_attempts: 3,
    };

    let request = ConflictResolutionRequest {
        mode: "conflict-resolution".to_string(),
        patch_id: "PATCH-001".to_string(),
        intent: "Keep the patch active.".to_string(),
        behavior_assertions: vec!["patch stays active".to_string()],
        original_diff: "diff".to_string(),
        upstream_diff: "upstream".to_string(),
        new_source: "source".to_string(),
    };

    let response: ConflictResolutionResponse = agent::send_request(&config, &request).unwrap();
    assert_eq!(response.confidence, 0.91);
    assert_eq!(response.notes, "Stable stub response.");
    assert_eq!(response.unresolved, Vec::<String>::new());
    assert_eq!(agent::provider_kind(&config), agent::ProviderKind::Exec);
    assert_eq!(
        agent::provider_kind(&AgentConfig {
            provider: String::new(),
            endpoint: "https://example.com/agent".to_string(),
            confidence_threshold: 0.8,
            max_attempts: 3,
        }),
        agent::ProviderKind::Http
    );
    assert_eq!(
        agent::provider_kind(&AgentConfig {
            provider: String::new(),
            endpoint: String::new(),
            confidence_threshold: 0.8,
            max_attempts: 3,
        }),
        agent::ProviderKind::None
    );

    let oversized = "x".repeat(agent::MAX_SOURCE_BYTES + 64);
    let (truncated, was_truncated) = agent::truncate_source(&oversized);
    assert!(was_truncated);
    assert!(truncated.contains("[truncated by t3-tape]"));
}

#[test]
fn update_with_exec_agent_stages_pending_review_and_approval_rewrites_state() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\n",
        "agent-conflict",
        "Keep the forked line change when upstream rewrites the same line.",
    );

    let diff_before =
        fs::read_to_string(pair.fork.join(".t3/patch/patches/PATCH-001.diff")).unwrap();
    let head_before = git_output(&pair.fork, &["rev-parse", "HEAD"]);
    let to_ref = commit_change(
        &pair.upstream,
        "src/app.txt",
        "alpha\nupstream\n",
        "upstream conflict",
    );

    let endpoint = write_exec_agent(
        &pair.temp,
        "conflict-agent",
        &json!({
            "resolved-diff": conflict_resolution_diff(),
            "confidence": 0.93,
            "notes": "Reapplied the fork intent against the upstream rewrite.",
            "unresolved": [],
        }),
    );
    configure_exec_agent(&pair.fork, &endpoint, 0.80);

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .success()
        .stdout(predicate::str::contains("pending-review"));

    let triage_path = pair.fork.join(".t3/patch/triage.json");
    let triage_before_approval = read_json(&triage_path);
    assert_eq!(
        triage_before_approval["patches"][0]["triage-status"],
        "pending-review"
    );
    assert_eq!(
        triage_before_approval["patches"][0]["detected-status"],
        "CONFLICT"
    );
    assert!(
        triage_before_approval["patches"][0]["apply-commit"]
            .as_str()
            .unwrap()
            .len()
            > 6
    );

    let resolved_diff_path = PathBuf::from(
        triage_before_approval["patches"][0]["resolved-diff-path"]
            .as_str()
            .unwrap(),
    );
    let notes_path = PathBuf::from(
        triage_before_approval["patches"][0]["notes-path"]
            .as_str()
            .unwrap(),
    );
    let raw_response_path = PathBuf::from(
        triage_before_approval["patches"][0]["raw-response-path"]
            .as_str()
            .unwrap(),
    );
    assert!(resolved_diff_path.is_file());
    assert!(notes_path.is_file());
    assert!(raw_response_path.is_file());

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["triage", "approve", "PATCH-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-001\tactive\tCOMPLETE"));

    assert_eq!(git_output(&pair.fork, &["rev-parse", "HEAD"]), head_before);

    let diff_after = fs::read_to_string(pair.fork.join(".t3/patch/patches/PATCH-001.diff")).unwrap();
    assert_ne!(diff_before, diff_after);
    assert!(diff_after.contains("+patched"));
    assert!(diff_after.contains("-upstream"));

    let meta = read_json(&pair.fork.join(".t3/patch/patches/PATCH-001.meta.json"));
    assert_eq!(meta["base-ref"], to_ref);
    assert_eq!(meta["current-ref"], to_ref);
    assert_eq!(meta["apply-confidence"], json!(0.93));
    let parsed = UnifiedDiff::parse(&diff_after).unwrap();
    assert_eq!(meta["surface-hash"], json!(surface_hash::compute(&parsed)));

    let patch_md = fs::read_to_string(pair.fork.join(".t3/patch.md")).unwrap();
    assert!(patch_md.contains(&format!("> base-ref: {to_ref}")));

    let migration_log = fs::read_to_string(pair.fork.join(".t3/patch/migration.log")).unwrap();
    assert!(migration_log.contains("COMPLETE"));

    let triage_after_approval = read_json(&triage_path);
    assert_eq!(triage_after_approval["patches"][0]["approved"], true);

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::eq("OK\n"));
}

#[test]
fn preview_failure_blocks_approval_but_keeps_artifacts() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\n",
        "preview-failure",
        "Keep the forked line change even when preview fails.",
    );

    let diff_before =
        fs::read_to_string(pair.fork.join(".t3/patch/patches/PATCH-001.diff")).unwrap();
    let to_ref = commit_change(
        &pair.upstream,
        "src/app.txt",
        "alpha\nupstream\n",
        "upstream conflict",
    );

    let endpoint = write_exec_agent(
        &pair.temp,
        "preview-agent",
        &json!({
            "resolved-diff": conflict_resolution_diff(),
            "confidence": 0.91,
            "notes": "Resolved for preview failure coverage.",
            "unresolved": [],
        }),
    );
    configure_exec_agent(&pair.fork, &endpoint, 0.80);
    configure_preview_command(&pair.fork, failing_preview_command());

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .success()
        .stdout(predicate::str::contains("pending-review"));

    let triage_before_approval = read_json(&pair.fork.join(".t3/patch/triage.json"));
    assert_eq!(triage_before_approval["preview"]["exit-code"], 7);
    let stdout_path = PathBuf::from(
        triage_before_approval["preview"]["stdout-path"]
            .as_str()
            .unwrap(),
    );
    let stderr_path = PathBuf::from(
        triage_before_approval["preview"]["stderr-path"]
            .as_str()
            .unwrap(),
    );
    assert!(stdout_path.is_file());
    assert!(stderr_path.is_file());

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["triage", "approve", "PATCH-001"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("sandbox preview failed"));

    let diff_after = fs::read_to_string(pair.fork.join(".t3/patch/patches/PATCH-001.diff")).unwrap();
    assert_eq!(diff_before, diff_after);
}

#[test]
fn rederive_promotes_missing_surface_to_pending_review() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\n",
        "missing-surface",
        "Keep the patch intent when the original file disappears upstream.",
    );

    let to_ref = delete_and_commit(&pair.upstream, "src/app.txt", "remove tracked file");

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .failure()
        .code(3)
        .stdout(predicate::str::contains("NEEDS-YOU"));

    let endpoint = write_exec_agent(
        &pair.temp,
        "rederive-agent",
        &json!({
            "derived-diff": rederive_diff(),
            "confidence": 0.92,
            "scope-update": {
                "files": ["src/app.txt"],
                "components": [],
            },
            "notes": "Recreated the missing surface from intent.",
            "unresolved": [],
        }),
    );
    configure_exec_agent(&pair.fork, &endpoint, 0.80);

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["rederive", "PATCH-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pending-review"));

    let triage_after = read_json(&pair.fork.join(".t3/patch/triage.json"));
    assert_eq!(
        triage_after["patches"][0]["detected-status"],
        "MISSING-SURFACE"
    );
    assert_eq!(
        triage_after["patches"][0]["triage-status"],
        "pending-review"
    );
    assert!(
        triage_after["patches"][0]["apply-commit"]
            .as_str()
            .unwrap()
            .len()
            > 6
    );
}

#[test]
fn update_applies_dependencies_in_requires_order() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\nfoundation\n",
        "base-patch",
        "Apply the foundation change before dependent customization.",
    );
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\nfeature\n",
        "dependent-patch",
        "Apply after the foundation patch.",
    );
    set_patch_requires(&pair.fork, "PATCH-002", &["PATCH-001"]);

    let to_ref = commit_change(&pair.upstream, "README.md", "# upstream\n", "docs only");

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-002\tdependent-patch\tCLEAN"));

    let triage = read_json(&pair.fork.join(".t3/patch/triage.json"));
    assert_eq!(triage["patches"][0]["id"], "PATCH-001");
    assert_eq!(triage["patches"][0]["detected-status"], "CLEAN");
    assert_eq!(triage["patches"][1]["id"], "PATCH-002");
    assert_eq!(triage["patches"][1]["detected-status"], "CLEAN");
    assert_eq!(triage["patches"][1]["triage-status"], "CLEAN");
    assert_eq!(triage["patches"][1]["dependency-blockers"], json!([]));
    assert!(triage["patches"][1]["apply-commit"].as_str().unwrap().len() > 6);
}

#[test]
fn update_blocks_dependent_patch_when_required_patch_is_unresolved() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\nfoundation\n",
        "base-patch",
        "Apply the foundation change before dependent customization.",
    );
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\nfeature\n",
        "dependent-patch",
        "Apply after the foundation patch.",
    );
    set_patch_requires(&pair.fork, "PATCH-002", &["PATCH-001"]);

    let to_ref = commit_change(
        &pair.upstream,
        "src/app.txt",
        "alpha\nupstream\n",
        "rewrite dependent surface",
    );

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref, "--ci"])
        .assert()
        .failure()
        .code(3)
        .stdout(predicate::str::contains("BLOCKED"));

    let triage = read_json(&pair.fork.join(".t3/patch/triage.json"));
    assert_eq!(triage["patches"][0]["detected-status"], "CONFLICT");
    assert_eq!(triage["patches"][0]["triage-status"], "NEEDS-YOU");
    assert_eq!(triage["patches"][1]["detected-status"], "BLOCKED");
    assert_eq!(triage["patches"][1]["triage-status"], "NEEDS-YOU");
    assert_eq!(
        triage["patches"][1]["dependency-blockers"],
        json!(["PATCH-001"])
    );
    assert!(triage["patches"][1]["apply-commit"].is_null());
}

#[test]
fn partial_approval_keeps_global_base_until_cycle_completion() {
    let pair = RepoPair::new();
    run_init(&pair.fork, &pair.upstream);
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\nbase\n",
        "first-clean-patch",
        "Keep the main file customization active.",
    );
    add_patch_and_commit(
        &pair.fork,
        "src/app.txt",
        "alpha\npatched\nbase\nextra\n",
        "second-clean-patch",
        "Keep the follow-up customization active.",
    );

    let base_before_update = patch_md_base_ref(&pair.fork);
    let to_ref = commit_change(&pair.upstream, "README.md", "# upstream\n", "docs only");

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["update", "--ref", &to_ref])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATCH-002\tsecond-clean-patch\tCLEAN"));

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["triage", "approve", "PATCH-001"])
        .assert()
        .success()
        .stdout(predicate::eq("PATCH-001\tactive\n"));

    assert_eq!(patch_md_base_ref(&pair.fork), base_before_update);
    let meta_one = read_json(&pair.fork.join(".t3/patch/patches/PATCH-001.meta.json"));
    let meta_two = read_json(&pair.fork.join(".t3/patch/patches/PATCH-002.meta.json"));
    assert_eq!(meta_one["base-ref"], to_ref);
    assert_eq!(meta_one["current-ref"], to_ref);
    assert_ne!(meta_two["base-ref"], json!(to_ref));

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::eq("OK\n"));

    t3_tape_command()
        .current_dir(&pair.fork)
        .args(["triage", "approve", "PATCH-002"])
        .assert()
        .success()
        .stdout(predicate::eq("PATCH-002\tactive\tCOMPLETE\n"));

    assert_eq!(patch_md_base_ref(&pair.fork), to_ref);
    let migration_log = fs::read_to_string(pair.fork.join(".t3/patch/migration.log")).unwrap();
    assert_eq!(migration_log.matches("status:   COMPLETE").count(), 1);
}
