//! The Switch Model screen: type to search, pick a model, slide the effort.
//!
//! Drawn in place under the prompt, not on an alternate screen. The rows are
//! reserved up front and every frame repaints those same rows, so the
//! terminal never scrolls under the screen and what was above stays readable.

use std::io::{self, Write};

use ccl_core::config::EffortLevel;
use crossterm::cursor::{self, Hide, MoveTo, MoveUp, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};

/// Model rows shown at once; the rest scroll.
const MAX_ROWS: usize = 10;
/// Every line of the screen except the model rows: header, warning, title,
/// blank, search, underline, blank | blank, 3 slider lines, blank, 1M, blank,
/// footer.
const CHROME_LINES: usize = 15;
/// The slider track starts after `  Effort  ◂  `.
const TRACK_INDENT: usize = 13;
/// Room between two effort stops, shrunk on narrow terminals.
const MAX_GAP: usize = 9;
const MIN_GAP: usize = 3;
/// Where the search text starts: after `  Search: `.
const SEARCH_INDENT: usize = 10;
const SEARCH_UNDERLINE: usize = 28;
/// 256-colour values, so terminals without true colour (macOS Terminal)
/// still show the blue of the picked model and the pink of the slider.
const ACCENT_MODEL: Color = Color::AnsiValue(75);
const ACCENT_EFFORT: Color = Color::AnsiValue(204);

pub struct Input<'a> {
    /// Who and where: key, gateway, folder.
    pub header: String,
    /// The ids the key may use, in display order.
    pub models: &'a [String],
    /// The folder's main model without `[1m]`, if it has one.
    pub current: Option<&'a str>,
    pub efforts: &'a [EffortLevel],
    /// Where the slider starts.
    pub effort: usize,
    pub long_context: bool,
    pub supports_long_context: &'a dyn Fn(&str) -> bool,
}

pub struct Pick {
    pub model: String,
    /// `None` when config.json offers no effort levels.
    pub effort: Option<String>,
    pub long_context: bool,
}

type Line = Vec<StyledContent<String>>;

struct State<'a> {
    input: Input<'a>,
    rows: usize,
    query: String,
    /// Indexes into `input.models` that match the query.
    matches: Vec<usize>,
    /// Index into `matches`.
    selected: usize,
    scroll: usize,
    effort: usize,
    long_context: bool,
}

enum Action {
    Stay,
    Quit,
    Pick,
}

/// Show the screen until Enter (`Some`) or Esc / Ctrl-C (`None`).
pub fn run(input: Input) -> io::Result<Option<Pick>> {
    let mut out = io::stdout();
    let rows = visible_rows(input.models.len());
    let height = CHROME_LINES + rows;

    // Reserve the space first, so painting never scrolls the terminal.
    print!("{}", "\n".repeat(height));
    out.flush()?;
    execute!(out, MoveUp(height as u16))?;
    let (_, top) = cursor::position()?;

    terminal::enable_raw_mode()?;
    let _restore = Restore;
    execute!(out, Hide)?;

    let mut state = State::new(input, rows);
    let picked = loop {
        draw(&mut out, &state, top)?;
        // Anything but a key press (a resize, a release on Windows) just redraws.
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match state.handle(key) {
            Action::Stay => {}
            Action::Quit => break None,
            Action::Pick => break state.pick(),
        }
    };
    execute!(out, MoveTo(0, top), Clear(ClearType::FromCursorDown))?;
    Ok(picked)
}

/// Puts the cursor and line discipline back however the screen ends.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show);
        let _ = terminal::disable_raw_mode();
    }
}

fn visible_rows(models: usize) -> usize {
    let height = terminal::size().map_or(24, |(_, h)| h as usize);
    let room = height.saturating_sub(CHROME_LINES + 1).max(3);
    models.clamp(1, MAX_ROWS).min(room)
}

impl<'a> State<'a> {
    fn new(input: Input<'a>, rows: usize) -> Self {
        let selected = input
            .current
            .and_then(|current| {
                input
                    .models
                    .iter()
                    .position(|id| id.eq_ignore_ascii_case(current))
            })
            .unwrap_or(0);
        let mut state = State {
            effort: input.effort.min(input.efforts.len().saturating_sub(1)),
            long_context: input.long_context,
            matches: (0..input.models.len()).collect(),
            input,
            rows,
            query: String::new(),
            selected,
            scroll: 0,
        };
        state.keep_visible();
        state
    }

    fn selected_id(&self) -> Option<&str> {
        self.matches
            .get(self.selected)
            .map(|&i| self.input.models[i].as_str())
    }

    /// The folder's model when the key cannot use it, so the screen can say so.
    fn missing_current(&self) -> Option<&str> {
        self.input.current.filter(|current| {
            !self
                .input
                .models
                .iter()
                .any(|id| id.eq_ignore_ascii_case(current))
        })
    }

    /// Re-run the search, keeping the highlighted model when it still matches.
    fn refilter(&mut self) {
        let keep = self.selected_id().map(str::to_string);
        let query = self.query.to_lowercase();
        let terms: Vec<&str> = query.split_whitespace().collect();
        self.matches = (0..self.input.models.len())
            .filter(|&i| {
                let id = self.input.models[i].to_lowercase();
                terms.iter().all(|term| id.contains(term))
            })
            .collect();
        self.selected = keep
            .and_then(|keep| {
                self.matches
                    .iter()
                    .position(|&i| self.input.models[i] == keep)
            })
            .unwrap_or(0);
        self.scroll = 0;
        self.keep_visible();
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.matches.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
        self.keep_visible();
    }

    fn keep_visible(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + self.rows {
            self.scroll = self.selected + 1 - self.rows;
        }
    }

    fn handle(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => return Action::Quit,
            KeyCode::Char('p') if ctrl => self.move_by(-1),
            KeyCode::Char('n') if ctrl => self.move_by(1),
            KeyCode::Char('u') if ctrl => {
                self.query.clear();
                self.refilter();
            }
            // Esc first clears a search, then leaves.
            KeyCode::Esc if self.query.is_empty() => return Action::Quit,
            KeyCode::Esc => {
                self.query.clear();
                self.refilter();
            }
            KeyCode::Enter if !self.matches.is_empty() => return Action::Pick,
            KeyCode::Up => self.move_by(-1),
            KeyCode::Down => self.move_by(1),
            KeyCode::PageUp => self.move_by(-(self.rows as isize)),
            KeyCode::PageDown => self.move_by(self.rows as isize),
            KeyCode::Left => self.effort = self.effort.saturating_sub(1),
            KeyCode::Right => {
                self.effort = (self.effort + 1).min(self.input.efforts.len().saturating_sub(1));
            }
            KeyCode::Tab => self.long_context = !self.long_context,
            KeyCode::Backspace => {
                self.query.pop();
                self.refilter();
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.refilter();
            }
            _ => {}
        }
        Action::Stay
    }

    fn pick(&self) -> Option<Pick> {
        Some(Pick {
            model: self.selected_id()?.to_string(),
            effort: self.input.efforts.get(self.effort).map(|e| e.id.clone()),
            long_context: self.long_context,
        })
    }
}

fn draw(out: &mut impl Write, state: &State, top: u16) -> io::Result<()> {
    // One column short of the edge: writing the last one wraps some terminals.
    let width = terminal::size()
        .map_or(80, |(w, _)| w as usize)
        .saturating_sub(1);
    for (i, line) in render(state, width).into_iter().enumerate() {
        queue!(
            out,
            MoveTo(0, top + i as u16),
            Clear(ClearType::CurrentLine)
        )?;
        for span in fit(line, width) {
            queue!(out, Print(span))?;
        }
    }
    out.flush()
}

fn plain(text: impl Into<String>) -> StyledContent<String> {
    text.into().stylize()
}

fn dim(text: impl Into<String>) -> StyledContent<String> {
    text.into().dark_grey()
}

/// Cut a line to `width` characters, so nothing wraps and shifts the rows.
fn fit(line: Line, width: usize) -> Line {
    let mut room = width;
    let mut fitted = Vec::with_capacity(line.len());
    for span in line {
        let len = span.content().chars().count();
        if len <= room {
            room -= len;
            fitted.push(span);
        } else {
            let cut: String = span.content().chars().take(room).collect();
            fitted.push(StyledContent::new(*span.style(), cut));
            break;
        }
    }
    fitted
}

fn render(state: &State, width: usize) -> Vec<Line> {
    let input = &state.input;
    let mut lines: Vec<Line> = Vec::with_capacity(CHROME_LINES + state.rows);

    lines.push(vec![dim(input.header.as_str())]);
    lines.push(match state.missing_current() {
        Some(id) => {
            vec![format!("! {id} is set here, but this key cannot use it; pick another").yellow()]
        }
        None => Vec::new(),
    });
    lines.push(vec!["Switch Model".to_string().bold()]);
    lines.push(Vec::new());

    let position = match state.matches.len() {
        0 => "no match".to_string(),
        n => format!("{}/{n}", state.selected + 1),
    };
    lines.push(vec![
        plain("  Search: "),
        plain(state.query.as_str()),
        dim("█"),
        dim(format!("   {position}")),
    ]);
    lines.push(vec![
        plain(" ".repeat(SEARCH_INDENT)),
        dim("─".repeat(SEARCH_UNDERLINE)),
    ]);
    lines.push(Vec::new());

    let id_width = input
        .models
        .iter()
        .map(|id| id.chars().count())
        .max()
        .unwrap_or(0);
    for row in 0..state.rows {
        let at = state.scroll + row;
        let mut line = match state.matches.get(at) {
            Some(&i) => {
                let id = &input.models[i];
                let mut line = if at == state.selected {
                    vec![format!("  > {id:<id_width$}").with(ACCENT_MODEL).bold()]
                } else {
                    vec![plain(format!("    {id:<id_width$}"))]
                };
                if input
                    .current
                    .is_some_and(|current| current.eq_ignore_ascii_case(id))
                {
                    line.push(dim("  (current)"));
                }
                line
            }
            None if row == 0 => vec![dim(format!("    No model matches \"{}\"", state.query))],
            None => Vec::new(),
        };
        if row == 0 && state.scroll > 0 {
            line.push(dim("  ↑ more"));
        }
        if row + 1 == state.rows && at + 1 < state.matches.len() {
            line.push(dim("  ↓ more"));
        }
        lines.push(line);
    }
    lines.push(Vec::new());

    lines.extend(slider(input.efforts, state.effort, width));
    lines.push(Vec::new());

    let supported = state
        .selected_id()
        .is_some_and(|id| (input.supports_long_context)(id));
    lines.push(if supported {
        vec![
            plain("  1M context  "),
            if state.long_context {
                "on".to_string().with(ACCENT_EFFORT).bold()
            } else {
                dim("off")
            },
            dim("   Tab to switch"),
        ]
    } else {
        vec![plain("  1M context  "), dim("not available for this model")]
    });
    lines.push(Vec::new());
    lines.push(vec![dim(
        "  ↑↓ model   ←→ effort   Tab 1M   type to search   Enter save   Esc quit",
    )]);

    debug_assert_eq!(lines.len(), CHROME_LINES + state.rows);
    lines
}

/// The effort slider: track, labels under each stop, and the description.
fn slider(efforts: &[EffortLevel], selected: usize, width: usize) -> [Line; 3] {
    let n = efforts.len();
    if n == 0 {
        return [
            vec![
                plain("  Effort  "),
                dim("none offered; add effort_levels with: ccl config edit"),
            ],
            Vec::new(),
            Vec::new(),
        ];
    }
    let gap = match n {
        1 => 0,
        _ => (width.saturating_sub(TRACK_INDENT + 4) / (n - 1))
            .saturating_sub(1)
            .clamp(MIN_GAP, MAX_GAP),
    };

    let arrow = |text: &str, live: bool| if live { plain(text) } else { dim(text) };
    let mut track = vec![plain("  Effort  "), arrow("◂", selected > 0), plain("  ")];
    for i in 0..n {
        if i > 0 {
            let segment = "─".repeat(gap);
            track.push(if i <= selected {
                segment.with(ACCENT_EFFORT)
            } else {
                dim(segment)
            });
        }
        track.push(match i.cmp(&selected) {
            std::cmp::Ordering::Less => "●".to_string().with(ACCENT_EFFORT),
            std::cmp::Ordering::Equal => "◉".to_string().with(ACCENT_EFFORT).bold(),
            std::cmp::Ordering::Greater => dim("○"),
        });
    }
    track.push(plain("  "));
    track.push(arrow("▸", selected + 1 < n));

    // Each label centred under its stop, never overlapping the one before.
    let mut labels = Vec::new();
    let mut column = 0;
    for (i, level) in efforts.iter().enumerate() {
        let stop = TRACK_INDENT + i * (gap + 1);
        let start = stop
            .saturating_sub(level.id.chars().count() / 2)
            .max(column);
        labels.push(plain(" ".repeat(start - column)));
        labels.push(if i == selected {
            level.id.clone().with(ACCENT_EFFORT).bold()
        } else {
            dim(level.id.as_str())
        });
        column = start + level.id.chars().count() + 1;
        labels.push(plain(" "));
    }

    let description = vec![
        plain(" ".repeat(TRACK_INDENT - 3)),
        efforts[selected].description.clone().dark_grey().italic(),
    ];
    [track, labels, description]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccl_core::config::Config;

    const MODELS: [&str; 8] = [
        "claude-opus-5-5-vn",
        "claude-opus-5-vn",
        "claude-opus-4-8-vn",
        "claude-sonnet-5-5-vn",
        "claude-sonnet-5-vn",
        "claude-sonnet-4-6-vn",
        "claude-haiku-5-5-vn",
        "claude-haiku-4-5-vn",
    ];

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn text(line: &Line) -> String {
        line.iter().map(|span| span.content().as_str()).collect()
    }

    fn with_state(check: impl FnOnce(&mut State)) {
        let models: Vec<String> = MODELS.iter().map(|m| m.to_string()).collect();
        let config = Config::default();
        let sonnet_or_opus = |id: &str| !id.contains("haiku");
        let input = Input {
            header: "key: work  |  gateway: gw  |  folder: demo".into(),
            models: &models,
            current: Some("claude-sonnet-5-vn"),
            efforts: &config.effort_levels,
            effort: 2,
            long_context: true,
            supports_long_context: &sonnet_or_opus,
        };
        let mut state = State::new(input, MAX_ROWS.min(models.len()));
        check(&mut state);
    }

    #[test]
    fn opens_on_the_folder_model_and_keeps_its_height() {
        with_state(|state| {
            assert_eq!(state.selected_id(), Some("claude-sonnet-5-vn"));
            let lines = render(state, 100);
            assert_eq!(lines.len(), CHROME_LINES + state.rows);
            let screen: Vec<String> = lines.iter().map(text).collect();
            assert!(
                screen
                    .iter()
                    .any(|l| l.contains("> claude-sonnet-5-vn") && l.contains("(current)"))
            );
        });
    }

    #[test]
    fn each_effort_label_sits_under_its_stop() {
        let config = Config::default();
        for selected in 0..config.effort_levels.len() {
            let [track, labels, _] = slider(&config.effort_levels, selected, 100);
            let track: Vec<char> = text(&track).chars().collect();
            let labels = text(&labels);
            let stop = track.iter().position(|&c| c == '◉').unwrap();
            let label = &config.effort_levels[selected].id;
            let start = labels.find(label.as_str()).unwrap();
            let start = labels[..start].chars().count();
            assert!(
                start <= stop && stop < start + label.len(),
                "{label}: stop {stop}, label at {start}"
            );
        }
    }

    #[test]
    fn typing_filters_and_arrows_set_the_effort() {
        with_state(|state| {
            for c in "opus 4".chars() {
                state.handle(key(KeyCode::Char(c)));
            }
            assert_eq!(state.selected_id(), Some("claude-opus-4-8-vn"));
            state.handle(key(KeyCode::Right));
            state.handle(key(KeyCode::Tab));
            assert!(matches!(state.handle(key(KeyCode::Enter)), Action::Pick));
            let pick = state.pick().unwrap();
            assert_eq!(pick.model, "claude-opus-4-8-vn");
            assert_eq!(pick.effort.as_deref(), Some("xhigh"));
            assert!(!pick.long_context);
        });
    }

    #[test]
    fn esc_clears_the_search_before_it_quits() {
        with_state(|state| {
            state.handle(key(KeyCode::Char('z')));
            assert!(state.matches.is_empty());
            assert!(matches!(state.handle(key(KeyCode::Enter)), Action::Stay));
            assert!(matches!(state.handle(key(KeyCode::Esc)), Action::Stay));
            assert_eq!(state.matches.len(), MODELS.len());
            assert!(matches!(state.handle(key(KeyCode::Esc)), Action::Quit));
        });
    }

    /// `cargo test -p ccl screen -- --nocapture` prints the screen as drawn.
    #[test]
    fn screen() {
        with_state(|state| {
            for line in render(state, 90) {
                println!("{}", text(&line));
            }
        });
    }
}
