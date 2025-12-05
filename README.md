# Arborist

**Automatic git worktree and branch management for LLM CLI workflows**

Arborist is a command wrapper that automatically manages git worktrees and branches when working with LLM-powered CLI tools like Claude Code. It provides isolated workspaces for each LLM session, preventing conflicts and maintaining a clean git history.

## The Problem

When using LLM CLI tools like Claude Code, you often want to:
- Keep experimental changes isolated from your main branch
- Run multiple LLM sessions simultaneously without conflicts
- Automatically clean up temporary branches when they're no longer needed
- Maintain a clean working directory while testing ideas

Manually managing branches and worktrees for each session is tedious and error-prone.

## The Solution

Arborist wraps any command (intended for agent CLIs like `claude`) and automatically:
1. Creates an isolated git worktree with a unique name
2. Executes your command in that isolated environment
3. Cleans up automatically if no changes were made
4. Keeps the branch/worktree if you made commits or have uncommitted changes

## Installation

### Build from source

```bash
git clone https://github.com/jpiccari/arborist-rs && cd arborist-rs
cargo install --path .
```

## Usage

### Basic Usage

Simply prefix any command with `arborist`:

```bash
arborist claude
```

This will:
- Create a new branch named `arborist/{color}` (e.g., `arborist/blue`)
- Launch Claude Code in that branch
- Clean up the branch automatically when you exit (if no changes were made)

### With Options

```bash
# Enable verbose output to see what's happening
arborist -v claude

# Use random color selection (instead of deterministic based on parent PID)
arborist -r claude

# Both options together
arborist -vr claude
```

### Command-Line Options

- `-v, --verbose`: Enable verbose output showing git operations
- `-r, --random`: Use random color selection for branch names
- `--help`: Show help information
- `--version`: Show version information

## How It Works

### Normal Repositories

When you run `arborist` in a normal git repository:

1. Detects your current branch and commit
2. Creates a new branch `arborist/{color}` from that commit
3. Checks out the new branch
4. Executes your command
5. After command exits:
   - If you made commits or have uncommitted changes: keeps the branch
   - If the branch is clean: deletes it and returns to your original branch

### Bare Repositories

When you run `arborist` in a bare repository (common for server-side repos):

1. Creates a new worktree at `{repo-root}/arborist-{color}`
2. Creates a branch `arborist/{color}` in that worktree
3. Changes to the worktree directory
4. Executes your command
5. After command exits:
   - If you made commits or have uncommitted changes: keeps the worktree
   - If clean: removes the worktree and deletes the branch

### Non-Git Directories

If you run `arborist` in a directory that's not a git repository, it simply executes the command directly without any git management.

## Use Cases

### Claude Code Sessions

Propping up the AI industry by running agents in parallel

```bash
# Imagine these commands run in separate terminal/tmux windows
arborist claude  # worktree on branch arborist/lime
arborist claude  # worktree on branch arborist/cyan
arborist claude  # worktree on branch arborist/magenta
...
```

### Other LLM CLIs

Works with any command-line tool:

```bash
# Run an isolated kiro session
arborist kiro-cli

# Launch dedicated worktrees for VSCode IDE sessions
arborist code --wait .

# Custom scripts for use-case I'm not even aware of...
arborist ./my-script.sh
```

## Branch Naming

Arborist uses colorful, memorable branch names from a palette of 28 colors. By default, color selection
is deterministic based on your terminal's parent process ID, so each terminal session consistently gets
the same color. Use `-r` for random selection instead.

## Integrations for your consideration

Add a function to your shell configuration (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
function ac() {
   arborist claude "$@"
}
```

Now you have quick access to be able to delete all your code in its own worktree!

```bash
ac -p 'delete all my code. then commit with an obnoxious message'
```

And get automatic branch management every time.

### Always-on Arborist: The nuclear option

If you want arborist to manage all your Claude Code sessions by default, you can set up a shell function:

```bash
function claude() {
    arborist claude "$@"
}
```

This intercepts all `claude` commands and wraps them with arborist.


### Merge changes before exiting

```bash
function and_merge() {
   "$@"
   claude -p "commit all changes. merge with the tracking branch by change to the tracking branch worktree and using `git pull`"
}

function acm() {
   arborist and_merge claude "$@"
}
```

With the above configuration in your shell configuration, you can then run commands like below to run isolated
agentic coding sessions. In this example, when claude finishes with the spec based refactoring, it will commit
and attempt to merge into the original branch. When arborist sees there are no changes and the worktree is not
ahead of the original brnach, it will seamlessly cleanup the worktree and branch created at the start of the
session.
```bash
acm -p 'use @awesome-feature-spec.md to refactor the code base'
```

## License

MIT because I cut my teeth on BSD kernel hacking and MIT is close enough.

## Contributing

Don't even think about it, I won't respond. If you want to contribute, I recommend you just take the
code, while adhering to the license, and compete directly with this project. If its good, I'll be a user.
