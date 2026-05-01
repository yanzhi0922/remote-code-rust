use thiserror::Error;

/// Errors that can occur during checkpoint operations.
#[derive(Error, Debug)]
pub enum CheckpointError {
    #[error("Shadow git repo already initialized")]
    AlreadyInitialized,

    #[error("Shadow git repo not initialized")]
    NotInitialized,

    #[error("Cannot use checkpoints in {0}")]
    ProtectedPath(String),

    #[error(
        "Checkpoints are disabled because a nested git repository was detected at: {0}. \
         Please remove or relocate nested git repositories to use the checkpoints feature."
    )]
    NestedGitRepo(String),

    #[error("Checkpoints can only be used in the original workspace: {expected} !== {actual}")]
    WorktreeMismatch { expected: String, actual: String },

    #[error("Checkpoints require core.worktree to be set in the shadow git config")]
    MissingWorktreeConfig,

    #[error("Git error: {0}")]
    GitError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for CheckpointError {
    fn from(err: std::io::Error) -> Self {
        CheckpointError::IoError(err.to_string())
    }
}

impl From<git2::Error> for CheckpointError {
    fn from(err: git2::Error) -> Self {
        CheckpointError::GitError(err.message().to_string())
    }
}
