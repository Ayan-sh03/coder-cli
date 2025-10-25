use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use crate::types::Message;

pub struct TuiApp {
    pub running: bool,
    pub input_mode: InputMode,
    pub messages: Vec<Message>,
    pub input: String,
    pub command: String,
    pub current_agent_response: Option<String>,
    pub status_line: String,
    pub show_help: bool,
    pub show_sidebar: bool,
    pub sidebar_tab: SidebarTab,
}

#[derive(PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
    Command,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SidebarTab {
    Tasks,
    Context,
    Shortcuts,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            running: true,
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            input: String::new(),
            command: String::new(),
            current_agent_response: None,
            status_line: "Normal: i=insert, :=command, ?=help, Tab=sidebar, q=quit".to_string(),
            show_help: false,
            show_sidebar: false,
            sidebar_tab: SidebarTab::Tasks,
        }
    }

    pub fn run(&mut self, messages: Vec<Message>) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        self.messages = messages;

        while self.running {
            terminal.draw(|f| ui(f, self))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_keypress(key)?;
                }
            }
        }

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn handle_keypress(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Editing;
                    self.status_line = "Insert: Enter=send, Esc=normal, Ctrl+L=clear".to_string();
                }
                KeyCode::Char(':') => {
                    self.input_mode = InputMode::Command;
                    self.command.clear();
                    self.status_line = "Command: type and Enter to run, Esc=normal".to_string();
                }
                KeyCode::Char('?') => {
                    self.show_help = !self.show_help;
                }
                KeyCode::Tab => {
                    self.show_sidebar = !self.show_sidebar;
                }
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('r') => self.current_agent_response = None,
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter => {
                    self.messages.push(Message {
                        role: "user".to_string(),
                        content: Some(self.input.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                    self.input.clear();
                    self.input_mode = InputMode::Normal;
                    self.status_line = "Processing... (Esc to cancel view)".to_string();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'l' | 'L' => self.input.clear(),
                            _ => {}
                        }
                    } else {
                        self.input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.status_line = "Normal: i=insert, :=command, ?=help, Tab=sidebar, q=quit".to_string();
                }
                _ => {}
            },
            InputMode::Command => match key.code {
                KeyCode::Enter => {
                    let cmd = self.command.trim().to_string();
                    self.execute_command(cmd);
                    self.command.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.command.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.status_line = "Normal: i=insert, :=command, ?=help, Tab=sidebar, q=quit".to_string();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'l' | 'L' => self.command.clear(),
                            _ => {}
                        }
                    } else {
                        self.command.push(c);
                    }
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn execute_command(&mut self, cmd: String) {
        // Simple command parser inspired by minimal CLIs
        let mut parts = cmd.split_whitespace();
        let name = parts.next().unwrap_or("");
        match name {
            "q" | "quit" | "exit" => {
                self.running = false;
            }
            "help" | "?" => {
                self.show_help = true;
                self.status_line = "Help opened. Press ? or Esc to close.".to_string();
            }
            "sidebar" => {
                self.show_sidebar = !self.show_sidebar;
                self.status_line = if self.show_sidebar {
                    "Sidebar shown (Tab toggles).".to_string()
                } else {
                    "Sidebar hidden.".to_string()
                };
            }
            "clear" => {
                self.messages.clear();
                self.status_line = "Messages cleared.".to_string();
            }
            "send" => {
                let rest: String = parts.collect::<Vec<_>>().join(" ");
                if rest.is_empty() {
                    self.status_line = "Nothing to send.".to_string();
                } else {
                    self.messages.push(Message {
                        role: "user".to_string(),
                        content: Some(rest),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                    self.status_line = "Sent.".to_string();
                }
            }
            "status" => {
                let rest: String = parts.collect::<Vec<_>>().join(" ");
                if rest.is_empty() {
                    self.status_line = "Status: (no text provided)".to_string();
                } else {
                    self.status_line = rest;
                }
            }
            _ if name.is_empty() => {
                self.status_line = "Command: (empty)".to_string();
            }
            _ => {
                self.status_line = format!("Unknown command: {}", name);
            }
        }
    }
}

fn ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_header(f, chunks[0]);
    render_main_content(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);

    // overlays last so they draw on top
    render_help_overlay(f, app, f.area());
}

fn render_header(f: &mut Frame, area: ratatui::layout::Rect) {
    let header_text = vec![
        Line::from(vec![
            Span::styled(
                "Rust Coding Agent",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled("glm-4.5-air", Style::default().fg(Color::Yellow)),
            Span::raw(" | "),
            Span::styled("Connected", Style::default().fg(Color::Green)),
        ]),
    ];

    let header = Paragraph::new(header_text).wrap(Wrap { trim: true });

    f.render_widget(header, area);
}

fn render_main_content(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    // Upper: messages (+ optional sidebar)
    if app.show_sidebar {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(vertical[0]);
        render_messages(f, app, horizontal[0]);
        render_sidebar(f, app, horizontal[1]);
    } else {
        render_messages(f, app, vertical[0]);
    }
    // Lower: input area
    render_input(f, app, vertical[1]);
}

fn render_messages(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .filter_map(|msg| {
            let content = msg.content.as_ref()?;
            let role_style = match msg.role.as_str() {
                "user" => Style::default().fg(Color::Yellow),
                "assistant" => Style::default().fg(Color::Cyan),
                "system" => Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                _ => Style::default(),
            };

            let role_prefix = match msg.role.as_str() {
                "user" => "You: ",
                "assistant" => "Agent: ",
                "system" => "System: ",
                _ => "",
            };

            let mut text = Text::default();
            text.lines.push(Line::from(vec![
                Span::styled(role_prefix, role_style.add_modifier(Modifier::BOLD)),
                Span::styled(content, Style::default().fg(Color::White)),
            ]));

            Some(ListItem::new(text))
        })
        .collect();

    let messages_list = List::new(messages);

    f.render_widget(messages_list, area);
}

fn render_sidebar(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let title = match app.sidebar_tab {
        SidebarTab::Tasks => "Tasks",
        SidebarTab::Context => "Context",
        SidebarTab::Shortcuts => "Shortcuts",
    };
    let content = match app.sidebar_tab {
        SidebarTab::Tasks => vec![
            Line::from(Span::styled("Tasks", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("") ,
            Line::from("- No active tasks"),
        ],
        SidebarTab::Context => vec![
            Line::from(Span::styled("Context", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("") ,
            Line::from(format!("Messages: {}", app.messages.len())),
        ],
        SidebarTab::Shortcuts => vec![
            Line::from(Span::styled("Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from("i: insert  | Esc: normal"),
            Line::from(": command   | ?: help"),
            Line::from("Tab: sidebar| q: quit"),
            Line::from("Ctrl+L: clear input"),
        ],
    };
    let para = Paragraph::new(content).wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let input_text = match app.input_mode {
        InputMode::Editing => vec![Line::from(vec![
            Span::styled("You: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&app.input, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::White)),
        ])],
        InputMode::Command => vec![Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(&app.command, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::White)),
        ])],
        InputMode::Normal => vec![Line::from(vec![
            Span::styled("Normal mode. i=insert, :=command, ?=help, Tab=sidebar, q=quit", Style::default().fg(Color::Gray)),
        ])],
    };

    let input_paragraph = Paragraph::new(input_text).wrap(Wrap { trim: true });

    f.render_widget(input_paragraph, area);
}

fn render_footer(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let footer_text = vec![
        Line::from(vec![
            Span::styled(&app.status_line, Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::styled(
                format!("Messages: {}", app.messages.len()),
                Style::default().fg(Color::Gray),
            ),
        ]),
    ];

    let footer = Paragraph::new(footer_text).wrap(Wrap { trim: true });

    f.render_widget(footer, area);
}

fn render_help_overlay(f: &mut Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    if !app.show_help { return; }
    let overlay_area = centered_rect_percentage(80, 60, area);
    let help_lines = vec![
        Line::from(Span::styled("Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("Modes:"),
        Line::from("  Normal: navigate and trigger commands"),
        Line::from("  Insert: type message, Enter to send"),
        Line::from("  Command: ':' then type, Enter to run"),
        Line::from(""),
        Line::from("Keys:"),
        Line::from("  i / Esc / : / ? / Tab / q / Ctrl+L"),
        Line::from(""),
        Line::from("Close with '?' or Esc from Normal/Command."),
    ];
    let para = Paragraph::new(help_lines).wrap(Wrap { trim: true });
    f.render_widget(para, overlay_area);
}

fn centered_rect_percentage(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

pub fn run_tui_session(messages: Vec<Message>) -> Result<()> {
    let mut app = TuiApp::new();
    app.run(messages)
}