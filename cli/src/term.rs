//! Small terminal pieces every command shares: status marks, prompts that
//! read a key without echoing it, and a note shown while the network works.

use std::fmt::Display;
use std::io::{self, BufRead, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::MoveToColumn;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::style::{Attribute, Color};
use crossterm::terminal::{self, Clear, ClearType};

/// Off when output is piped: crossterm's own switch drops colours but still
/// writes bold and reset codes into the text.
static COLOR: AtomicBool = AtomicBool::new(true);

pub fn set_color(on: bool) {
    COLOR.store(on, Ordering::Relaxed);
}

fn paint(
    text: &(impl Display + ?Sized),
    color: Option<Color>,
    attribute: Option<Attribute>,
) -> String {
    use crossterm::style::Stylize as _;
    let text = text.to_string();
    if !COLOR.load(Ordering::Relaxed) {
        return text;
    }
    let mut styled = text.stylize();
    if let Some(color) = color {
        styled = styled.with(color);
    }
    if let Some(attribute) = attribute {
        styled = styled.attribute(attribute);
    }
    styled.to_string()
}

/// crossterm's `Stylize` for printed output, honouring [`set_color`]. The
/// picker draws only on a terminal and uses crossterm's directly.
pub trait Paint: Display {
    fn green(&self) -> String {
        paint(self, Some(Color::Green), None)
    }
    fn red(&self) -> String {
        paint(self, Some(Color::Red), None)
    }
    fn yellow(&self) -> String {
        paint(self, Some(Color::Yellow), None)
    }
    fn cyan(&self) -> String {
        paint(self, Some(Color::Cyan), None)
    }
    fn dark_grey(&self) -> String {
        paint(self, Some(Color::DarkGrey), None)
    }
    fn bold(&self) -> String {
        paint(self, None, Some(Attribute::Bold))
    }
}

impl<T: Display + ?Sized> Paint for T {}

#[derive(Debug, Clone, Copy)]
pub enum Mark {
    Ok,
    Warn,
    Fail,
    Info,
}

impl Mark {
    fn symbol(self) -> String {
        match self {
            Mark::Ok => "✓".green(),
            Mark::Warn => "!".yellow(),
            Mark::Fail => "✗".red(),
            Mark::Info => "·".dark_grey(),
        }
    }
}

/// One status line: `  ✓ label    text`.
pub fn row(mark: Mark, label: &str, text: impl Display) {
    println!("  {} {label:<8} {text}", mark.symbol());
}

/// The host of a URL, which is all a status line needs.
pub fn host(url: &str) -> &str {
    let url = url.trim();
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    url.split('/').next().unwrap_or(url)
}

/// Run `work` with `note` on screen, cleared once it is done.
pub fn waiting<T>(note: &str, work: impl FnOnce() -> T) -> T {
    let mut err = io::stderr();
    let tty = err.is_terminal();
    if tty {
        let _ = write!(err, "{}", note.dark_grey());
        let _ = err.flush();
    }
    let result = work();
    if tty {
        let _ = execute!(err, MoveToColumn(0), Clear(ClearType::CurrentLine));
    }
    result
}

pub fn interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// A line of input; `default` when it is left empty.
pub fn prompt(label: &str, default: &str) -> io::Result<String> {
    eprint!("{label}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let line = line.trim();
    Ok(if line.is_empty() { default } else { line }.to_string())
}

/// A yes/no question that defaults to no.
pub fn confirm(question: &str) -> io::Result<bool> {
    let answer = prompt(&format!("{question} [y/N] "), "n")?;
    Ok(matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes"))
}

/// Restores the terminal however the raw-mode section ends.
struct RawMode;

impl RawMode {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Read a secret, showing `*` per character so a paste visibly landed.
/// `None` when cancelled with Esc or Ctrl-C. Piped input is read as a line.
pub fn secret(label: &str) -> io::Result<Option<String>> {
    if !io::stdin().is_terminal() {
        let line = prompt(label, "")?;
        return Ok((!line.is_empty()).then_some(line));
    }
    let mut err = io::stderr();
    write!(err, "{label}")?;
    err.flush()?;
    let mut value = String::new();
    let raw = RawMode::enter()?;
    let cancelled = loop {
        let Event::Key(key) = event::read()? else {
            continue;
        };
        // Windows reports releases too; acting on them would double every key.
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Enter => break false,
            KeyCode::Esc => break true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break true,
            KeyCode::Backspace => {
                if value.pop().is_some() {
                    write!(err, "\x08 \x08")?;
                }
            }
            KeyCode::Char(c) => {
                value.push(c);
                write!(err, "*")?;
            }
            _ => {}
        }
        err.flush()?;
    };
    drop(raw);
    writeln!(err)?;
    let value = value.trim().to_string();
    Ok((!cancelled && !value.is_empty()).then_some(value))
}
