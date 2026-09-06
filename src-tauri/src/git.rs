//! Plain host-side git operations (shells out to `git` directly, not `sbx`).
//! Used for "Git Sync": fetching a clone-mode sandbox's auto-wired
//! `sandbox-<name>` git-daemon remote and fast-forwarding any local branch
//! that can take the update cleanly.

use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("git command failed: {0}")]
  CommandFailed(String),
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

fn run(repo_path: &str, args: &[&str]) -> Result<String> {
  let output = Command::new("git").current_dir(repo_path).args(args).output()?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    return Err(Error::CommandFailed(format!("git {args:?} exited with {:?}: {stderr}", output.status.code())));
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn ref_exists(repo_path: &str, r#ref: &str) -> bool {
  run(repo_path, &["rev-parse", "--verify", "--quiet", r#ref]).is_ok()
}

fn is_ancestor(repo_path: &str, ancestor: &str, descendant: &str) -> Result<bool> {
  let status =
    Command::new("git").current_dir(repo_path).args(["merge-base", "--is-ancestor", ancestor, descendant]).status()?;
  Ok(status.success())
}

/// The repo's currently checked-out branch, or `None` on detached HEAD (or
/// any other `symbolic-ref` failure, e.g. `repo_path` isn't a git repo at
/// all). Used both internally (`sync_branch`) and by branch-7's
/// `base_branch` snapshot at sandbox creation time.
pub fn current_branch(repo_path: &str) -> Option<String> {
  run(repo_path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchSyncStatus {
  FastForwarded,
  NeedsManualMerge,
  NewBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BranchSyncOutcome {
  pub branch: String,
  pub status: BranchSyncStatus,
}

/// Decides and applies the outcome for one branch, given the local branch
/// name and the ref it should catch up to (a fetched remote-tracking ref in
/// production, any ref in tests). Never force-updates: fast-forwards only
/// when `branch` is an ancestor of `target_ref`, otherwise just reports.
pub fn sync_branch(repo_path: &str, branch: &str, target_ref: &str) -> Result<BranchSyncOutcome> {
  let local_ref = format!("refs/heads/{branch}");
  if !ref_exists(repo_path, &local_ref) {
    return Ok(BranchSyncOutcome { branch: branch.to_string(), status: BranchSyncStatus::NewBranch });
  }

  if !is_ancestor(repo_path, &local_ref, target_ref)? {
    return Ok(BranchSyncOutcome { branch: branch.to_string(), status: BranchSyncStatus::NeedsManualMerge });
  }

  if current_branch(repo_path).as_deref() == Some(branch) {
    run(repo_path, &["merge", "--ff-only", target_ref])?;
  } else {
    run(repo_path, &["fetch", ".", &format!("{target_ref}:{branch}")])?;
  }
  Ok(BranchSyncOutcome { branch: branch.to_string(), status: BranchSyncStatus::FastForwarded })
}

/// `git fetch sandbox-<name>` then fast-forward-or-report every branch under
/// the resulting `refs/remotes/sandbox-<name>/*`. Never pushes, never force-
/// updates, never creates a local branch for a brand-new remote one.
pub fn sync_from_sandbox(repo_path: &str, sbx_name: &str) -> Result<Vec<BranchSyncOutcome>> {
  let remote = format!("sandbox-{sbx_name}");
  run(repo_path, &["fetch", &remote])?;

  let prefix = format!("refs/remotes/{remote}/");
  let listing = run(repo_path, &["for-each-ref", "--format=%(refname)", &prefix])?;

  let mut outcomes = Vec::new();
  for line in listing.lines() {
    let Some(branch) = line.strip_prefix(&prefix) else { continue };
    if branch == "HEAD" {
      continue; // remote's symbolic HEAD, not a real branch
    }
    let target_ref = format!("{remote}/{branch}");
    outcomes.push(sync_branch(repo_path, branch, &target_ref)?);
  }
  Ok(outcomes)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  fn init_repo() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("overnight-git-sync-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.to_str().unwrap();
    run(path, &["init", "-q", "-b", "main"]).unwrap();
    run(path, &["config", "user.email", "test@example.com"]).unwrap();
    run(path, &["config", "user.name", "Test"]).unwrap();
    dir
  }

  fn commit(repo: &str, file: &str, contents: &str) -> String {
    fs::write(format!("{repo}/{file}"), contents).unwrap();
    run(repo, &["add", "."]).unwrap();
    run(repo, &["commit", "-q", "-m", "commit"]).unwrap();
    run(repo, &["rev-parse", "HEAD"]).unwrap()
  }

  #[test]
  fn clean_fast_forward_updates_non_checked_out_branch() {
    let dir = init_repo();
    let repo = dir.to_str().unwrap();
    commit(repo, "a.txt", "1");
    run(repo, &["branch", "feature"]).unwrap(); // feature == main so far
    let ahead = commit(repo, "a.txt", "2"); // main moves ahead of feature

    let outcome = sync_branch(repo, "feature", "main").unwrap();
    assert_eq!(outcome, BranchSyncOutcome { branch: "feature".to_string(), status: BranchSyncStatus::FastForwarded });
    assert_eq!(run(repo, &["rev-parse", "feature"]).unwrap(), ahead);

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn clean_fast_forward_updates_currently_checked_out_branch() {
    let dir = init_repo();
    let repo = dir.to_str().unwrap();
    commit(repo, "a.txt", "1");
    run(repo, &["checkout", "-q", "-b", "feature"]).unwrap();
    run(repo, &["checkout", "-q", "main"]).unwrap();
    let ahead = commit(repo, "a.txt", "2");
    run(repo, &["checkout", "-q", "feature"]).unwrap();

    let outcome = sync_branch(repo, "feature", "main").unwrap();
    assert_eq!(outcome.status, BranchSyncStatus::FastForwarded);
    assert_eq!(run(repo, &["rev-parse", "feature"]).unwrap(), ahead);
    assert_eq!(current_branch(repo).as_deref(), Some("feature"));

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn diverged_branch_is_reported_and_left_untouched() {
    let dir = init_repo();
    let repo = dir.to_str().unwrap();
    commit(repo, "a.txt", "1");
    run(repo, &["checkout", "-q", "-b", "feature"]).unwrap();
    let feature_head = commit(repo, "b.txt", "feature-only");
    run(repo, &["checkout", "-q", "main"]).unwrap();
    commit(repo, "c.txt", "main-only");

    let outcome = sync_branch(repo, "feature", "main").unwrap();
    assert_eq!(
      outcome,
      BranchSyncOutcome { branch: "feature".to_string(), status: BranchSyncStatus::NeedsManualMerge }
    );
    // untouched: feature still points at its own commit, not main's.
    assert_eq!(run(repo, &["rev-parse", "feature"]).unwrap(), feature_head);

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn current_branch_is_none_on_detached_head() {
    let dir = init_repo();
    let repo = dir.to_str().unwrap();
    let head = commit(repo, "a.txt", "1");
    assert_eq!(current_branch(repo).as_deref(), Some("main"));

    run(repo, &["checkout", "-q", &head]).unwrap();
    assert_eq!(current_branch(repo), None);

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn no_local_branch_is_reported_as_new_without_creating_one() {
    let dir = init_repo();
    let repo = dir.to_str().unwrap();
    commit(repo, "a.txt", "1");
    run(repo, &["checkout", "-q", "-b", "brand-new"]).unwrap();
    commit(repo, "b.txt", "2");
    run(repo, &["checkout", "-q", "main"]).unwrap();

    let outcome = sync_branch(repo, "does-not-exist-locally", "brand-new").unwrap();
    assert_eq!(
      outcome,
      BranchSyncOutcome { branch: "does-not-exist-locally".to_string(), status: BranchSyncStatus::NewBranch }
    );
    assert!(!ref_exists(repo, "refs/heads/does-not-exist-locally"));

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn sync_from_sandbox_lists_and_fast_forwards_across_a_real_remote() {
    let upstream_dir = init_repo();
    let upstream = upstream_dir.to_str().unwrap();
    commit(upstream, "a.txt", "1");

    // Local repo starts as a real clone (shared history, not just similar
    // content), named the way sync_from_sandbox expects: sandbox-<sbx_name>.
    let local_dir = std::env::temp_dir().join(format!("overnight-git-sync-{}", uuid::Uuid::new_v4()));
    let local = local_dir.to_str().unwrap();
    let tmp = std::env::temp_dir();
    run(tmp.to_str().unwrap(), &["clone", "-q", "--origin", "sandbox-test", upstream, local]).unwrap();

    let ahead = commit(upstream, "a.txt", "2"); // sandbox moves ahead of the host's local main

    let outcomes = sync_from_sandbox(local, "test").unwrap();
    assert_eq!(
      outcomes,
      vec![BranchSyncOutcome { branch: "main".to_string(), status: BranchSyncStatus::FastForwarded }]
    );
    assert_eq!(run(local, &["rev-parse", "main"]).unwrap(), ahead);

    fs::remove_dir_all(upstream_dir).unwrap();
    fs::remove_dir_all(local_dir).unwrap();
  }
}
