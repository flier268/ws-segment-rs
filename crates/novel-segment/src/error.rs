//! Error type.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Library error.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    DictNotFound { name: String },
    InvalidDictLine { path: PathBuf, line: String },
    InvalidInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::DictNotFound { name } => write!(f, "cannot find dict file `{name}`"),
            Error::InvalidDictLine { path, line } => {
                write!(f, "invalid dict line in {}: {line}", path.display())
            }
            Error::InvalidInput(s) => write!(f, "invalid input: {s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
