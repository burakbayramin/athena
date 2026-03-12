//! Integration tests for ath-git using tempfile repos.
//!
//! These helpers and tests validate the GitLayer foundation.
//! Commit/staging tests are added in Plan 02.

use std::path::Path;

use ath_git::GitLayer;
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
