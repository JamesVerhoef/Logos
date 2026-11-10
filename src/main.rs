use ratatui::{DefaultTerminal, Frame, crossterm::{cursor::SetCursorStyle, event::{self, Event, KeyCode, KeyEventKind}, execute}, layout::{Constraint, Layout}, widgets::{Block, Paragraph}};
use ropey::Rope;
use unicode_width::UnicodeWidthChar;
use clap::Parser;

const TAB: usize = 4;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();

    let Some(file) = cli.file else { return Ok(()); };

    color_eyre::install()?;
    ratatui::run(|t| App::new(file).run(t))
}

struct App {
    text: Rope,
    index: usize,
    actions: Vec<Action>,
    mode: Mode,
    current_file: String,
}

#[derive(Clone, PartialEq)]
enum Action {
    MoveCursor(Unit, isize),
    Write(String),
    Delete(Unit, isize),
    SetMode(Mode),
    SaveCurrent,
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Insert,
    Command,
}

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Char,
    Word,
    Line,
}

impl App {
    fn new(file: String) -> Self {
        let mut app = Self {
            text: Rope::new(),
            index: 0,
            actions: vec![],
            mode: Mode::Command,
            current_file: file,
        };

        app.switch_mode(Mode::Command).unwrap();
        let text = app.load_file();
        app.text = Rope::from_str(&text.unwrap());

        app
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                
                if matches!(self.mode, Mode::Insert) {
                    self.handle_action( match key.code {
                        KeyCode::Esc => Action::SetMode(Mode::Command),
                        KeyCode::Char(c) => Action::Write(c.to_string()),
                        KeyCode::Backspace => Action::Delete(Unit::Char, -1),
                        KeyCode::Delete => Action::Delete(Unit::Char, 1),
                        KeyCode::Left => Action::MoveCursor(Unit::Char, -1),
                        KeyCode::Right => Action::MoveCursor(Unit::Char, 1),
                        KeyCode::Up => Action::MoveCursor(Unit::Line, -1),
                        KeyCode::Down => Action::MoveCursor(Unit::Line, 1),
                        KeyCode::Enter => Action::Write("\n".to_string()),
                        _ => Action::None,
                    })
                } else if matches!(self.mode, Mode::Command) {
                    self.handle_action( match key.code {
                        KeyCode::Char('i') => Action::SetMode(Mode::Insert),
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('s') => Action::SaveCurrent,
                        _ => Action::None,
                    })
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        let [main, status] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
        ]).areas(frame.area());

        let block = Block::bordered().title("text buffer");
        let inner = block.inner(main);

        frame.render_widget(
            Paragraph::new(self.text.to_string()).block(block),
            main,
        );
        frame.render_widget(
            Paragraph::new(match self.mode {
                Mode::Insert => "INSERT",
                Mode::Command => "COMMAND",
            }),
            status,
        );

        let (line, col) = cursor_line_col(&self.text, self.index);

        let x = inner.x + col as u16;
        let y = inner.y + line as u16;

        if x < inner.right() && y < inner.bottom() {
            frame.set_cursor_position((x, y));
        }
    }

    fn handle_action(&mut self, action: Action) {
        if action == Action::None { return; }

        match &action {
            Action::Write(s) => {
                self.text.insert(self.index, s);
                self.index += s.chars().count();
            },
            Action::MoveCursor(Unit::Char, n) => {
                self.index = self.index.saturating_add_signed(*n).min(self.text.len_chars());
            },
            Action::MoveCursor(Unit::Line, n) => {
                let line = self.text.char_to_line(self.index);
                let col = self.index - self.text.line_to_char(line);

                let target = line.saturating_add_signed(*n).min(self.text.len_lines() - 1);
                let start = self.text.line_to_char(target);
                
                let mut len = self.text.line(target).len_chars();
                if target + 1 < self.text.len_lines() {
                    len -= 1;
                }

                self.index = start + col.min(len);
            },
            Action::Delete(Unit::Char, n) => {
                let target = self.index.saturating_add_signed(*n).min(self.text.len_chars());
                let (start, end) = (self.index.min(target), self.index.max(target));
                self.text.remove(start..end);
                self.index = start;
            },
            Action::SetMode(mode) => self.switch_mode(*mode).unwrap(),
            Action::SaveCurrent => self.write_file(),
            Action::None => {},
            _ => todo!(),
        }

        self.actions.push(action);
    }

    fn switch_mode(&mut self, mode: Mode) -> std::io::Result<()> {
        match mode {
            Mode::Command => {
                self.mode = Mode::Command;
                execute!(std::io::stdout(), SetCursorStyle::SteadyBlock)
            },
            Mode::Insert => {
                self.mode = Mode::Insert;
                execute!(std::io::stdout(), SetCursorStyle::SteadyBar)
            },
        }
    }

    fn write_file(&self) {
        std::fs::write(self.current_file.clone(), self.text.to_string()).unwrap();
    }

    fn load_file(&self) -> Option<String> {
        std::fs::read_to_string(self.current_file.clone()).ok()
    }
}

fn cursor_line_col(text: &Rope, index: usize) -> (usize, usize) {
    let line = text.char_to_line(index);
    let line_start = text.line_to_char(line);
    let col = text.slice(line_start..index).chars().fold(0, |col, c| {
        if c == '\t' { (col / TAB + 1) * TAB } else { col + c.width().unwrap_or(0) }
    });
    (line, col)
}
