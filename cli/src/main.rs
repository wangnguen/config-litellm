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
    /// Print the version
    Version,
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

    let result = match Cli::parse().command {
        None => commands::home::run(),
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
