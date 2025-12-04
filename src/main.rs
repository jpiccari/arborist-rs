mod error;
mod git;

use clap::Parser;
use duct::cmd;
use error::Result;
use rand::prelude::*;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

// Global verbose flag
static VERBOSE: AtomicBool = AtomicBool::new(false);

// Macro for verbose logging
macro_rules! verbose {
    ($($arg:tt)*) => {
        if VERBOSE.load(Ordering::Relaxed) {
            eprintln!($($arg)*);
        }
    };
}

// Color palette for random selection
const COLORS: &[&str] = &[
    "red",
    "blue",
    "green",
    "yellow",
    "purple",
    "orange",
    "pink",
    "cyan",
    "teal",
    "magenta",
    "violet",
    "amber",
    "crimson",
    "navy",
    "indigo",
    "lime",
    "coral",
    "maroon",
    "turquoise",
    "slate",
    "lavender",
    "mint",
    "peach",
    "ruby",
    "sapphire",
    "emerald",
    "topaz",
];

// CLI argument structure
#[derive(Parser, Debug)]
#[command(name = "arborist")]
#[command(about = "Automatically manage git worktrees and branches for command execution")]
#[command(version)]
struct Args {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Command and arguments to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    command: Vec<String>,
}

// Directory guard to restore original directory
struct DirectoryGuard {
    original: PathBuf,
}

impl DirectoryGuard {
    fn with_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let original = env::current_dir()?;
        env::set_current_dir(path)?;
        Ok(DirectoryGuard { original })
    }
}

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(e) = env::set_current_dir(&self.original) {
            verbose!("Warning: Failed to restore original directory: {}", e);
        }
    }
}

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {}", err);
            1
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    let args = Args::try_parse().unwrap_or_else(|e| e.exit());

    // Set global verbose flag
    VERBOSE.store(args.verbose, Ordering::Relaxed);

    // Step 1: Initialization
    verbose!("Checking repository...");
    let repo_info = git::get_repo_info()?;

    match repo_info {
        None => {
            // Step 2c: Non-git directory, just run command
            verbose!("Not a git repository, running command directly...");
            let exit_code = execute_shell_command(&args.command)?;
            return Ok(exit_code);
        }
        Some(repo) if repo.is_bare => {
            // Step 2a: Bare repository, use worktrees
            verbose!("Bare repository detected");
            verbose!("Repository: {}", repo.root.display());
            verbose!("Branch: {}", repo.current_branch);

            let color = select_color();
            let worktree_path = repo
                .root
                .join("worktrees")
                .join(format!("arborist-{}", &color));

            verbose!("Preparing worktree at: {}", worktree_path.display());

            // Check if worktree exists
            if git::worktree_exists(&worktree_path)? {
                verbose!("Worktree already exists, using existing worktree");
            }

            let branch_name = format!("arborist/{}", color);
            verbose!("Creating new worktree with branch '{}'...", branch_name);
            git::create_worktree(
                &worktree_path,
                &branch_name,
                &repo.current_commit,
                Some(&repo.current_branch),
            )?;

            // Change to worktree directory
            let prev_path = DirectoryGuard::with_path(&worktree_path)?;
            verbose!("Changed to worktree directory");

            // Execute user command
            let exit_code = execute_shell_command(&args.command)?;

            // Step 3: Cleanup
            verbose!("Checking worktree status...");
            let status = git::get_worktree_status()?;

            if status.has_changes {
                verbose!("Note: Uncommitted changes exist in worktree");
                verbose!("Keeping worktree at: {}", worktree_path.display());
            } else if status.commits_ahead > 0 {
                verbose!("Note: {} unpushed commit(s) exist", status.commits_ahead);
                verbose!("Keeping worktree at: {}", worktree_path.display());
            } else {
                verbose!("No changes detected, removing worktree...");
                // Return to original directory before removing worktree
                drop(prev_path);
                git::remove_worktree_and_branch(&worktree_path, &branch_name)?;
                verbose!("Worktree and branch removed");
            }

            Ok(exit_code)
        }
        Some(repo) => {
            // Step 2b: Normal repository, use branches
            verbose!("Normal repository detected");
            verbose!("Repository: {}", repo.root.display());
            verbose!("Current branch: {}", repo.current_branch);

            let color = select_color();
            let branch_name = format!("arborist/{}", color);

            verbose!("Preparing branch '{}'...", branch_name);

            // Check if branch exists
            if git::branch_exists(&branch_name)? {
                verbose!("Branch already exists, checking out...");
                git::checkout_branch(&branch_name)?;
            } else {
                verbose!("Creating new branch...");
                git::create_branch(&branch_name, &repo.current_commit)?;
            }

            // Execute user command (stay in current directory)
            let exit_code = execute_shell_command(&args.command)?;

            // Step 3: Cleanup
            verbose!("Checking branch status...");
            let status = git::get_worktree_status()?;

            if status.has_changes {
                verbose!(
                    "Note: Uncommitted changes exist in branch '{}'",
                    branch_name
                );
                verbose!("Keeping branch (remember to switch back manually)");
            } else if status.commits_ahead > 0 {
                verbose!(
                    "Note: {} unpushed commit(s) exist in branch '{}'",
                    status.commits_ahead,
                    branch_name
                );
                verbose!("Keeping branch (remember to switch back manually)");
            } else {
                verbose!("No changes detected, returning to original branch...");
                git::checkout_branch(&repo.current_branch)?;
                verbose!("Deleting branch '{}'...", branch_name);
                git::delete_branch(&branch_name)?;
            }

            Ok(exit_code)
        }
    }
}

// Execute shell command
fn execute_shell_command(command_args: &[String]) -> Result<i32> {
    if command_args.is_empty() {
        return Ok(0);
    }

    let program = &command_args[0];
    let args = &command_args[1..];

    let output = cmd(program, args).unchecked().run()?;

    let exit_code = output.status.code().unwrap_or(1);

    Ok(exit_code)
}

// Select a random color from the palette
fn select_color() -> String {
    let mut rng = rand::rng();
    COLORS
        .choose(&mut rng)
        .expect("Color palette should not be empty")
        .to_string()
}
