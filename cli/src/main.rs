//! `ccl`: point Claude Code at a LiteLLM gateway and pick its model with one
//! command. This binary is the terminal; everything it knows lives in
//! `ccl-core`.

mod commands;
mod picker;
mod term;

use std::io::{self, IsTerminal};

use clap::{Parser, Subcommand};
use term::Paint;

/// Set by the release build from the tag; a local build reports Cargo.toml's.
pub const VERSION: &str = match option_env!("CCL_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
/// Where releases are published; `update` and the install scripts read it.
pub const REPO: &str = "wangnguen/config-litellm";

#[derive(Parser)]
#[command(
    name = "ccl",
    version = VERSION,
    about = "Point Claude Code at a LiteLLM gateway and pick its model.\nRun without a command to open the model picker.",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List the models your key can use, and which slot uses each
    Ls,
    /// Call each slot's model for real and time the answer
    Check {
        /// Call every model the key can use, not only the four slots
        #[arg(long)]
        all: bool,
    },
    /// Manage saved keys (no action: list them)
    Key {
        #[command(subcommand)]
        action: Option<KeyAction>,
    },
    /// Print ~/.ccl/config.json, or open it in an editor
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Print this folder's Claude Code settings, key masked
    Show,
    /// Look for anything that overrides this folder's settings
    Doctor,
    /// Download and install the latest release
    Update,
    /// Remove ccl, its config and every saved key
    Uninstall,
    /// Print the version
    Version,
}

#[derive(Subcommand)]
pub enum KeyAction {
    /// List saved keys
    Ls,
    /// Save a key under a name (the key is asked for, hidden)
    Add { name: String },
    /// Use a saved key in this folder and make it the default
    Use { name: String },
    /// Delete a saved key
    Rm { name: String },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Open config.json in your editor
    Edit,
    /// Print where ccl keeps its files
    Path,
}

fn main() {
    // Old Windows consoles need VT processing switched on before colours work.
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();
    // Colour only on a terminal: piped or logged output gets plain text.
    if !io::stdout().is_terminal() || !io::stderr().is_terminal() {
        crossterm::style::force_color_output(false);
        term::set_color(false);
    }
    commands::update::clean_leftovers();

    let result = match Cli::parse().command {
        None => commands::home::run(),
        Some(Command::Ls) => commands::ls::run(),
        Some(Command::Check { all }) => commands::check::run(all),
        Some(Command::Key { action }) => commands::key::run(action),
        Some(Command::Config { action }) => commands::config::run(action),
        Some(Command::Show) => commands::show::run(),
        Some(Command::Doctor) => commands::doctor::run(),
        Some(Command::Update) => commands::update::run(),
        Some(Command::Uninstall) => commands::uninstall::run(),
        Some(Command::Version) => {
            println!("ccl {VERSION}");
            Ok(())
        }
    };
    if let Err(err) = result {
        eprintln!("{} {err:#}", "error:".red().bold());
        std::process::exit(1);
    }
}
