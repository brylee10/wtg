use thiserror::Error;

/// WTG Errors
#[derive(Debug, Error)]
pub enum WtgError {
    #[error("No command run yet in this session.")]
    NoCommandRun { logfile: String },
    #[error("Chat should have stdin connected to a tty, otherwise input is not interactive.")]
    ChatNotTty,
    #[error("Nix error: {0}")]
    NixError(#[from] nix::Error),
    #[error("Failed to open log file: {logfile}. Does it exist?")]
    LogFileOpenError { logfile: String },
    #[error(transparent)]
    StdioError(#[from] std::io::Error),
}
