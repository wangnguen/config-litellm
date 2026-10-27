//! The one error type of the core. Each variant asks something different of
//! the person reading it, which is all the CLI needs to say what to do next.

use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    /// Reading or writing a file failed.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A JSON file is there but is not valid JSON. It is never overwritten,
    /// so a typo in a hand-edited file costs a fix, not the file.
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    /// The gateway turned the key down.
    Rejected(String),
    /// The gateway could not be reached at all.
    Unreachable(String),
    /// The gateway answered, but not the way a LiteLLM proxy does.
    Unexpected(String),
    /// A request that cannot be met, such as a key name nobody saved.
    Invalid(String),
    /// No home directory to keep `~/.ccl` in.
    NoHome,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

// The cause is part of each message rather than a `source()`, so a caller
// printing the whole chain shows it once, not twice.
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Json { path, source } => write!(
                f,
                "{} is not valid JSON ({source}); fix it by hand, ccl will not overwrite it",
                path.display()
            ),
            Error::Rejected(msg) => write!(f, "the gateway rejected the key: {msg}"),
            Error::Unreachable(msg) => write!(f, "cannot reach the gateway: {msg}"),
            Error::Unexpected(msg) | Error::Invalid(msg) => f.write_str(msg),
            Error::NoHome => {
                f.write_str("cannot find your home directory (HOME / USERPROFILE is not set)")
            }
        }
    }
}

impl std::error::Error for Error {}
