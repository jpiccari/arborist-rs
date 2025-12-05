use crate::error::{ArboristError, Result};
use duct::cmd;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

// Helper function to run git commands and return stdout
fn run_git_cmd(args: &[&str]) -> Result<String> {
    let output = cmd("git", args)
        .stderr_capture()
        .stdout_capture()
        .unchecked()
        .run()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArboristError::GitOperationFailed(stderr.trim().to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// Helper function to safely convert Path to String
fn path_to_string(path: &Path) -> Result<String> {
    path.to_str()
        .ok_or_else(|| {
            ArboristError::InvalidPath(format!("Path contains non-UTF8 characters: {:?}", path))
        })
        .map(|s| s.to_string())
}

/// Computes the worktree path for a non-bare repository
/// Returns: /tmp/arborist/{sha256_hash}/{color}
pub fn compute_nonbare_worktree_path(repo_root: &Path, color: &str) -> Result<PathBuf> {
    let repo_path_str = path_to_string(repo_root)?;
    let mut hasher = Sha256::new();
    hasher.update(repo_path_str.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);

    let path = PathBuf::from("/tmp")
        .join("arborist")
        .join(hash_hex)
        .join(color);

    Ok(path)
}

/// Ensures the base directory for a worktree path exists
fn ensure_worktree_base_dir(worktree_path: &Path) -> Result<()> {
    if let Some(parent) = worktree_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                ArboristError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Failed to create worktree base directory {}: {}",
                        parent.display(),
                        e
                    ),
                ))
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub root: PathBuf,
    pub current_branch: String,
    pub current_commit: String,
    pub is_bare: bool,
}

#[derive(Debug, Clone)]
pub struct WorktreeStatus {
    pub has_changes: bool,
    pub commits_ahead: usize,
}

fn is_bare_repository() -> Result<bool> {
    // Get the common git directory (handles both normal repos and worktrees)
    // For worktrees, this points to the main repository's git directory
    let common_dir = run_git_cmd(&["rev-parse", "--git-common-dir"])?;

    // Check if the repository at the common directory is bare
    // This correctly identifies bare repos even when called from within a worktree
    let output = cmd(
        "git",
        &["-C", &common_dir, "rev-parse", "--is-bare-repository"],
    )
    .stderr_capture()
    .stdout_capture()
    .unchecked()
    .run()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArboristError::GitOperationFailed(stderr.trim().to_string()));
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(result == "true")
}

pub fn get_repo_info() -> Result<Option<GitRepo>> {
    if !is_git_repo()? {
        return Ok(None);
    }

    let root = get_repo_root()?;
    let current_branch = get_current_branch()?;
    let current_commit = get_current_commit()?;
    let is_bare = is_bare_repository()?;

    Ok(Some(GitRepo {
        root,
        current_branch,
        current_commit,
        is_bare,
    }))
}

fn is_git_repo() -> Result<bool> {
    let output = cmd!("git", "rev-parse", "--is-inside-work-tree")
        .stderr_capture()
        .stdout_capture()
        .unchecked()
        .run()?;

    Ok(output.status.success())
}

fn get_repo_root() -> Result<PathBuf> {
    // For bare repositories, --show-toplevel doesn't work, so we use --absolute-git-dir
    // For normal repositories, we use --show-toplevel to get the working tree root
    let is_bare = is_bare_repository()?;

    let path = if is_bare {
        // For bare repos, get the common git directory (the bare repo itself)
        // Then use its parent directory as the base for creating worktrees
        let git_dir = run_git_cmd(&["rev-parse", "--git-common-dir"])?;
        PathBuf::from(git_dir)
    } else {
        PathBuf::from(run_git_cmd(&["rev-parse", "--show-toplevel"])?)
    };

    Ok(path)
}

fn get_current_branch() -> Result<String> {
    run_git_cmd(&["rev-parse", "--abbrev-ref", "HEAD"])
}

fn get_current_commit() -> Result<String> {
    run_git_cmd(&["rev-parse", "HEAD"])
}

pub fn worktree_exists(path: &Path) -> Result<bool> {
    let output = run_git_cmd(&["worktree", "list"])?;
    let path_str = path_to_string(path)?;
    Ok(output.contains(&path_str))
}

pub fn create_worktree(
    path: &Path,
    branch: &str,
    commit: &str,
    upstream_branch: Option<&str>,
) -> Result<()> {
    // Ensure base directory exists (for non-bare repos in /tmp)
    ensure_worktree_base_dir(path)?;

    // If worktree already exists, skip creation
    if worktree_exists(path)? {
        return Ok(());
    }

    let path_str = path_to_string(path)?;
    let output = cmd!("git", "worktree", "add", "-b", branch, &path_str, commit)
        .stderr_capture()
        .stdout_capture()
        .unchecked()
        .run()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArboristError::GitOperationFailed(format!(
            "Failed to create worktree: {}",
            stderr
        )));
    }

    // Set upstream tracking branch if specified
    if let Some(upstream) = upstream_branch {
        let output = cmd!(
            "git",
            "-C",
            &path_str,
            "branch",
            "--set-upstream-to",
            upstream
        )
        .stderr_capture()
        .stdout_capture()
        .unchecked()
        .run()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArboristError::GitOperationFailed(format!(
                "Failed to set upstream tracking branch: {}",
                stderr
            )));
        }
    }

    Ok(())
}

pub fn remove_worktree(path: &Path) -> Result<()> {
    let path_str = path_to_string(path)?;
    let output = cmd!("git", "worktree", "remove", &path_str, "--force")
        .stderr_capture()
        .unchecked()
        .run()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArboristError::GitOperationFailed(format!(
            "Failed to remove worktree: {}",
            stderr
        )));
    }

    Ok(())
}

pub fn remove_worktree_and_branch(path: &Path, branch: &str) -> Result<()> {
    // First remove the worktree
    remove_worktree(path)?;

    // Then delete the associated branch
    delete_branch(branch)?;

    Ok(())
}

pub fn get_worktree_status() -> Result<WorktreeStatus> {
    let has_changes = has_uncommitted_changes()?;
    let commits_ahead = get_commits_ahead()?;

    Ok(WorktreeStatus {
        has_changes,
        commits_ahead,
    })
}

fn has_uncommitted_changes() -> Result<bool> {
    let output = run_git_cmd(&["status", "--porcelain"])?;
    Ok(!output.is_empty())
}

fn get_commits_ahead() -> Result<usize> {
    // Check if upstream exists
    match run_git_cmd(&["rev-parse", "--abbrev-ref", "@{upstream}"]) {
        Ok(_) => {
            // Get count of commits ahead
            let output = run_git_cmd(&["rev-list", "--count", "@{upstream}..HEAD"])?;
            Ok(output.parse().unwrap_or(0))
        }
        Err(_) => Ok(0), // No upstream = 0 ahead (intentional, not an error)
    }
}

pub fn delete_branch(branch: &str) -> Result<()> {
    run_git_cmd(&["branch", "-D", branch])?;
    Ok(())
}
