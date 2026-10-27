//! Everything ccl knows about a LiteLLM gateway and Claude Code's settings,
//! with no terminal in sight. The `ccl` binary is one shell over this crate;
//! nothing here prints, prompts or parses arguments.

pub mod config;
pub mod error;
mod file;
pub mod gateway;
pub mod keys;
pub mod models;
pub mod paths;
pub mod settings;

pub use error::{Error, Result};
