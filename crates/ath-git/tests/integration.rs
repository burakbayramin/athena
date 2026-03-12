//! Integration tests for ath-git using tempfile repos.
//!
//! Tests cover GitLayer foundation (Plan 01) and commit workflow (Plan 02).

use std::path::Path;

use ath_git::{CommitMetadata, GitError, GitLayer};
use tempfile::TempDir;

/// Create a temporary directory and initialize a GitLayer (auto-inits the repo).
fn create_temp_repo() -> (TempDir, GitLayer) {
    let dir = TempDir::new().expect("create tempdir");
    let layer = GitLayer::new(dir.path().to_path_buf()).expect("create GitLayer");
    (dir, layer)
}

/// Write a file at the given relative path inside a directory.
fn write_file(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(&path, content).expect("write file");
}

/// Create a dummy initial commit so the repo is non-empty.
///
/// This is needed for dirty-tree tests because `is_dirty()` returns false
/// for empty repos (no tracked files to compare against).
fn make_initial_commit(layer: &GitLayer) {
    // Access the repo through a new GitLayer to get the repo handle
    // We re-open the repo directly with git2 since GitLayer doesn't expose the mutex publicly
    let repo = git2::Repository::open(layer.repo_path()).expect("open repo");
    let sig = git2::Signature::now("Test", "test@test.com").expect("create signature");
    let mut index = repo.index().expect("get index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .expect("create initial commit");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn git_layer_opens_existing_repo() {
    let dir = TempDir::new().expect("create tempdir");
    // Init repo directly with git2
    git2::Repository::init(dir.path()).expect("git init");
    // GitLayer should open the existing repo
    let layer = GitLayer::new(dir.path().to_path_buf()).expect("open existing repo");
    assert_eq!(layer.repo_path(), dir.path());
}

#[test]
fn git_layer_auto_inits_new_repo() {
    let dir = TempDir::new().expect("create tempdir");
    assert!(!dir.path().join(".git").exists(), ".git should not exist yet");
    let layer = GitLayer::new(dir.path().to_path_buf()).expect("auto-init repo");
    assert!(dir.path().join(".git").exists(), ".git should exist after auto-init");
    assert_eq!(layer.repo_path(), dir.path());
}

#[test]
fn dirty_tree_with_untracked_file() {
    let (dir, layer) = create_temp_repo();
    make_initial_commit(&layer);

    // Write a new untracked file
    write_file(dir.path(), "new_file.txt", "hello world");

    assert!(
        layer.is_dirty().expect("is_dirty"),
        "repo should be dirty with an untracked file"
    );
}

#[test]
fn clean_repo_not_dirty() {
    let (_dir, layer) = create_temp_repo();
    make_initial_commit(&layer);

    // No modifications after initial commit
    assert!(
        !layer.is_dirty().expect("is_dirty"),
        "repo should be clean with no modifications"
    );
}

#[test]
fn empty_repo_not_dirty() {
    let (_dir, layer) = create_temp_repo();
    // Auto-init, no commits at all
    assert!(
        !layer.is_dirty().expect("is_dirty"),
        "empty repo (no commits) should not be considered dirty"
    );
}

// ---------------------------------------------------------------------------
// Plan 02: Commit workflow integration tests
// ---------------------------------------------------------------------------

fn sample_meta() -> CommitMetadata {
    CommitMetadata {
        phase_name: "test-phase".to_string(),
        agent_provider: "Anthropic".to_string(),
        agent_model: "opus-4".to_string(),
        task_id: "task-001".to_string(),
        files_count: 1,
        review_status: None,
        reviewer: None,
    }
}

#[test]
fn commit_contains_only_staged_files() {
    let (dir, layer) = create_temp_repo();

    // Create initial commit with file_a
    write_file(dir.path(), "file_a.txt", "aaa");
    layer
        .stage_and_commit(
            &[dir.path().join("file_a.txt")],
            &CommitMetadata {
                files_count: 1,
                ..sample_meta()
            },
        )
        .expect("initial commit");

    // Write file_b and file_c
    write_file(dir.path(), "file_b.txt", "bbb");
    write_file(dir.path(), "file_c.txt", "ccc");

    // Stage only file_b
    let result = layer
        .stage_and_commit(
            &[dir.path().join("file_b.txt")],
            &CommitMetadata {
                files_count: 1,
                ..sample_meta()
            },
        )
        .expect("second commit");

    assert!(result.committed);

    // Inspect the commit tree
    let repo = git2::Repository::open(dir.path()).expect("open repo");
    let commit = repo.find_commit(result.oid.unwrap()).unwrap();
    let tree = commit.tree().unwrap();

    assert!(
        tree.get_name("file_a.txt").is_some(),
        "file_a.txt should be in tree (from initial commit)"
    );
    assert!(
        tree.get_name("file_b.txt").is_some(),
        "file_b.txt should be in tree (newly staged)"
    );
    assert!(
        tree.get_name("file_c.txt").is_none(),
        "file_c.txt should NOT be in tree (not staged)"
    );
}

#[test]
fn commit_message_has_trailers() {
    let (dir, layer) = create_temp_repo();

    write_file(dir.path(), "file.txt", "content");
    let result = layer
        .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
        .expect("commit");

    let repo = git2::Repository::open(dir.path()).expect("open repo");
    let commit = repo.find_commit(result.oid.unwrap()).unwrap();
    let message = commit.message().unwrap();

    assert!(
        message.starts_with("athena:"),
        "subject should start with 'athena:'"
    );
    assert!(message.contains("Phase: test-phase"), "missing Phase trailer");
    assert!(
        message.contains("Agent: Anthropic/opus-4"),
        "missing Agent trailer"
    );
    assert!(message.contains("Task-Id: task-001"), "missing Task-Id trailer");
    assert!(
        message.contains("Files-Count: 1"),
        "missing Files-Count trailer"
    );
}

#[test]
fn initial_commit_empty_repo() {
    let (dir, layer) = create_temp_repo();
    // No prior commits -- repo is empty

    write_file(dir.path(), "init.txt", "hello");
    let result = layer
        .stage_and_commit(&[dir.path().join("init.txt")], &sample_meta())
        .expect("initial commit");

    assert!(result.committed, "should have created a commit");
    assert!(result.oid.is_some(), "should have an OID");

    // HEAD should now exist
    let repo = git2::Repository::open(dir.path()).expect("open repo");
    assert!(
        repo.head().is_ok(),
        "HEAD should exist after initial commit"
    );
}

#[test]
fn one_commit_per_phase() {
    let (dir, layer) = create_temp_repo();

    // First commit
    write_file(dir.path(), "a.txt", "aaa");
    layer
        .stage_and_commit(
            &[dir.path().join("a.txt")],
            &CommitMetadata {
                phase_name: "phase-1".to_string(),
                task_id: "t-1".to_string(),
                ..sample_meta()
            },
        )
        .expect("first commit");

    // Second commit
    write_file(dir.path(), "b.txt", "bbb");
    layer
        .stage_and_commit(
            &[dir.path().join("b.txt")],
            &CommitMetadata {
                phase_name: "phase-2".to_string(),
                task_id: "t-2".to_string(),
                ..sample_meta()
            },
        )
        .expect("second commit");

    // Count commits via revwalk
    let repo = git2::Repository::open(dir.path()).expect("open repo");
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    let count = revwalk.count();
    assert_eq!(count, 2, "should have exactly 2 commits in git log");
}

#[test]
fn empty_diff_skips_commit() {
    let (dir, layer) = create_temp_repo();

    // First commit
    write_file(dir.path(), "file.txt", "content");
    let first = layer
        .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
        .expect("first commit");
    assert!(first.committed);

    // Stage same file again (no changes)
    let second = layer
        .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
        .expect("second stage_and_commit");

    assert!(
        !second.committed,
        "should skip commit when diff is empty"
    );
    assert!(second.oid.is_none(), "no OID for skipped commit");
}

#[test]
fn missing_file_returns_error() {
    let (dir, layer) = create_temp_repo();

    let result = layer.stage_and_commit(
        &[dir.path().join("nonexistent.txt")],
        &sample_meta(),
    );

    assert!(result.is_err(), "should error on missing file");
    let err = result.unwrap_err();
    assert!(
        matches!(err, GitError::FileMissing { .. }),
        "expected GitError::FileMissing, got: {:?}",
        err
    );
}

#[test]
fn commit_with_review_trailers() {
    let (dir, layer) = create_temp_repo();

    write_file(dir.path(), "reviewed.txt", "code");

    let meta = CommitMetadata {
        review_status: Some("passed".to_string()),
        reviewer: Some("Google/2.5-pro".to_string()),
        ..sample_meta()
    };

    let result = layer
        .stage_and_commit(&[dir.path().join("reviewed.txt")], &meta)
        .expect("commit with review");

    let repo = git2::Repository::open(dir.path()).expect("open repo");
    let commit = repo.find_commit(result.oid.unwrap()).unwrap();
    let message = commit.message().unwrap();

    assert!(
        message.contains("Review-Status: passed"),
        "missing Review-Status trailer"
    );
    assert!(
        message.contains("Reviewer: Google/2.5-pro"),
        "missing Reviewer trailer"
    );
}
