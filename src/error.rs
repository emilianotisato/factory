//! One error type for the crate. Every variant says what the driver was doing
//! when it failed: a bare `ENOENT` from a run with forty files open is useless.
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    Json {
        what: String,
        source: serde_json::Error,
    },
    Toml {
        what: String,
        source: toml::de::Error,
    },
    Sql(rusqlite::Error),
    /// A subprocess (`docs.mjs`, `git`) exited non-zero or did not produce
    /// what was expected; `detail` quotes its stderr.
    Tool {
        cmd: String,
        detail: String,
    },
    Msg(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { what, source } => write!(f, "{what}: {source}"),
            Error::Json { what, source } => write!(f, "{what}: {source}"),
            Error::Toml { what, source } => write!(f, "{what}: {source}"),
            Error::Sql(e) => write!(f, "sqlite: {e}"),
            Error::Tool { cmd, detail } => write!(f, "`{cmd}` failed: {detail}"),
            Error::Msg(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sql(e)
    }
}

/// `.map_err(io("read state.json"))`.
pub fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

/// `.map_err(json("parse state.json"))`.
pub fn json(what: impl Into<String>) -> impl FnOnce(serde_json::Error) -> Error {
    let what = what.into();
    move |source| Error::Json { what, source }
}
