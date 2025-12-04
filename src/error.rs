use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ArboristError {
    GitOperationFailed(String),
    InvalidPath(String),
    IoError(io::Error),
}

impl fmt::Display for ArboristError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArboristError::GitOperationFailed(msg) => {
                write!(f, "Git operation failed: {}", msg)
            }
            ArboristError::InvalidPath(msg) => {
                write!(f, "Invalid path: {}", msg)
            }
            ArboristError::IoError(err) => {
                write!(f, "IO error: {}", err)
            }
        }
    }
}

impl std::error::Error for ArboristError {}

impl From<io::Error> for ArboristError {
    fn from(err: io::Error) -> Self {
        ArboristError::IoError(err)
    }
}

pub type Result<T> = std::result::Result<T, ArboristError>;
