use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame, Terminal,
};
use crate::types::Message;
use tokio::sync::mpsc;

// Catppuccin Mocha color palette
pub mod colors {
    use ratatui::style::Color;
    
    pub const BASE: Color = Color::Rgb(30, 30, 46);       // #1e1e2e
    pub const MANTLE: Color = Color::Rgb(24, 24, 37);     // #181825
    pub const CRUST: Color = Color::Rgb(17, 17, 27);      // #11111b
    pub const TEXT: Color = Color::Rgb(205, 214, 244);    // #cdd6f4
    pub const SUBTEXT1: Color = Color::Rgb(186, 194, 222); // #bac2de
    pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200); // #a6adc8
    pub const OVERLAY2: Color = Color::Rgb(147, 153, 178); // #9399b2
    pub const OVERLAY1: Color = Color::Rgb(127, 132, 156); // #7f849c
    pub const OVERLAY0: Color = Color::Rgb(108, 112, 134); // #6c7086
    pub const SURFACE2: Color = Color::Rgb(88, 91, 112);   // #585b70
    pub const SURFACE1: Color = Color::Rgb(69, 71, 90);    // #45475a
    pub const SURFACE0: Color = Color::Rgb(49, 50, 68);    // #313244
    
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254); // #b4befe
    pub const BLUE: Color = Color::Rgb(137, 180, 250);     // #89b4fa
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236); // #74c7ec
    pub const SKY: Color = Color::Rgb(137, 220, 235);      // #89dceb
    pub const TEAL: Color = Color::Rgb(148, 226, 213);     // #94e2d5
    pub const GREEN: Color = Color::Rgb(166, 227, 161);    // #a6e3a1
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);   // #f9e2af
    pub const PEACH: Color = Color::Rgb(250, 179, 135);    // #fab387
    pub const MAROON: Color = Color::Rgb(235, 160, 172);   // #eba0ac
    pub const RED: Color = Color::Rgb(243, 139, 168);      // #f38ba8
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);    // #cba6f7
    pub const PINK: Color = Color::Rgb(245, 194, 231);     // #f5c2e7
    pub const FLAMINGO: Color = Color::Rgb(242, 205, 205); // #f2cdcd
    pub const ROSEWATER: Color = Color::Rgb(245, 224, 220); // #f5e0dc
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    Input(String),
    AgentMessage(String),
    ToolCall(String, String), // name, args
    ToolResult(String),
    Error(String),
    StatusUpdate(String),
    Complete,
}

pub struct TuiApp {
    pub running: bool,
    pub input_mode: InputMode,
    pub messages: Vec<DisplayMessage>,
    pub input: String,
    pub status_line: String,
    pub scroll_offset: usize,
    pub show_help: bool,
    tx: mpsc::UnboundedSender<UiEvent>,
    rx: mpsc::UnboundedReceiver<UiEvent>,
}

#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

impl TuiApp {
    pub fn new() -> (Self, mpsc::UnboundedSender<UiEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let app = Self {
            running: true,
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            input: String::new(),
            status_line: "Ready | Press 'i' to type, 'q' to quit, '?' for help".to_string(),
            scroll_offset: 0,
            show_help: false,
            tx: tx.clone(),
            rx,
        };
        (app, tx)
    }

    pub async fn run_with_input_callback(&mut self, input_tx: mpsc::UnboundedSender<String>) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

        let result = self.run_loop_with_callback(&mut terminal, input_tx).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_loop_with_callback(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, input_tx: mpsc::UnboundedSender<String>) -> Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Non-blocking event handling
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_keypress_with_callback(key, &input_tx)?;
                    }
                }
            }

            // Process UI events
            while let Ok(event) = self.rx.try_recv() {
                self.handle_ui_event(event);
            }

            if !self.running {
                break;
            }
        }
        Ok(())
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::AgentMessage(content) => {
                // Check if we should append to existing assistant message
                if let Some(last) = self.messages.last_mut() {
                    if last.role == "assistant" {
                        last.content.push_str(&content);
                        self.auto_scroll();
                        return;
                    }
                }
                // Otherwise create new message
                self.messages.push(DisplayMessage {
                    role: "assistant".to_string(),
                    content: content.clone(),
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
                self.auto_scroll();
            }
            UiEvent::ToolCall(name, args) => {
                self.messages.push(DisplayMessage {
                    role: "tool".to_string(),
                    content: format!("🔧 {} {}", name, args),
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
                self.auto_scroll();
            }
            UiEvent::ToolResult(result) => {
                self.messages.push(DisplayMessage {
                    role: "tool_result".to_string(),
                    content: result,
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
                self.auto_scroll();
            }
            UiEvent::Error(error) => {
                self.messages.push(DisplayMessage {
                    role: "error".to_string(),
                    content: format!("❌ {}", error),
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
                self.status_line = format!("Error: {}", error);
                self.auto_scroll();
            }
            UiEvent::StatusUpdate(status) => {
                self.status_line = status;
            }
            UiEvent::Complete => {
                self.status_line = "✓ Complete | Press 'i' to continue".to_string();
            }
            UiEvent::Input(content) => {
                self.messages.push(DisplayMessage {
                    role: "user".to_string(),
                    content,
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
                self.auto_scroll();
            }
        }
    }

    fn auto_scroll(&mut self) {
        if self.messages.len() > 10 {
            self.scroll_offset = self.messages.len().saturating_sub(10);
        }
    }

    fn handle_keypress_with_callback(&mut self, key: KeyEvent, input_tx: &mpsc::UnboundedSender<String>) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Editing;
                    self.status_line = "Insert mode | Enter to send, Esc to cancel".to_string();
                }
                KeyCode::Char('?') => {
                    self.show_help = !self.show_help;
                }
                KeyCode::Char('q') => {
                    self.running = false;
                }
                KeyCode::Up => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                KeyCode::Down => {
                    if self.scroll_offset < self.messages.len().saturating_sub(1) {
                        self.scroll_offset += 1;
                    }
                }
                KeyCode::PageUp => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.scroll_offset = (self.scroll_offset + 10).min(self.messages.len().saturating_sub(1));
                }
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter => {
                    if !self.input.trim().is_empty() {
                        let input = self.input.clone();
                        
                        // Add to local display immediately
                        self.messages.push(DisplayMessage {
                            role: "user".to_string(),
                            content: input.clone(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        });
                        self.auto_scroll();
                        
                        // Send to agent
                        let _ = input_tx.send(input);
                        
                        self.input.clear();
                        self.input_mode = InputMode::Normal;
                        self.status_line = "Processing...".to_string();
                    }
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'c' => {
                                self.input.clear();
                                self.input_mode = InputMode::Normal;
                                self.status_line = "Cancelled".to_string();
                            }
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
                    self.status_line = "Ready | Press 'i' to type, 'q' to quit, '?' for help".to_string();
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(0),      // Messages
                Constraint::Length(3),   // Input
                Constraint::Length(3),   // Status
            ])
            .split(f.area());

        self.render_header(f, chunks[0]);
        self.render_messages(f, chunks[1]);
        self.render_input(f, chunks[2]);
        self.render_status(f, chunks[3]);

        if self.show_help {
            self.render_help_overlay(f, f.area());
        }
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let title = vec![
            Span::styled("● ", Style::default().fg(colors::GREEN)),
            Span::styled("Coding Agent", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(colors::SURFACE1)),
            Span::raw(" "),
            Span::styled("Powered by LLM", Style::default().fg(colors::SUBTEXT0)),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::SURFACE1))
            .style(Style::default().bg(colors::MANTLE));

        let paragraph = Paragraph::new(Line::from(title))
            .block(block)
            .style(Style::default().fg(colors::TEXT));

        f.render_widget(paragraph, area);
    }

    fn render_messages(&self, f: &mut Frame, area: Rect) {
        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .map(|(_i, msg)| {
                let (role_prefix, role_color, icon) = match msg.role.as_str() {
                    "user" => ("You", colors::BLUE, "❯"),
                    "assistant" => ("Agent", colors::MAUVE, "●"),
                    "tool" => ("Tool", colors::YELLOW, "🔧"),
                    "tool_result" => ("Result", colors::GREEN, "✓"),
                    "error" => ("Error", colors::RED, "✗"),
                    "system" => ("System", colors::OVERLAY0, "i"),
                    _ => ("Unknown", colors::TEXT, "?"),
                };

                let mut lines = vec![
                    Line::from(vec![
                        Span::styled(format!("{} ", icon), Style::default().fg(role_color)),
                        Span::styled(format!("{:8}", role_prefix), Style::default().fg(role_color).add_modifier(Modifier::BOLD)),
                        Span::styled(" │ ", Style::default().fg(colors::SURFACE1)),
                        Span::styled(&msg.timestamp, Style::default().fg(colors::OVERLAY0)),
                    ]),
                ];

                // Split content into multiple lines if needed
                let content_lines: Vec<&str> = msg.content.lines().collect();
                for (idx, line) in content_lines.iter().enumerate() {
                    let prefix = if idx == 0 { "  " } else { "  " };
                    lines.push(Line::from(vec![
                        Span::raw(prefix),
                        Span::styled(*line, Style::default().fg(colors::TEXT)),
                    ]));
                }

                // Add spacing between messages
                lines.push(Line::from(""));

                ListItem::new(Text::from(lines))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::SURFACE1))
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled("Messages", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
            ]))
            .style(Style::default().bg(colors::BASE));

        let list = List::new(messages)
            .block(block)
            .style(Style::default().fg(colors::TEXT));

        f.render_widget(list, area);

        // Render scrollbar
        let mut scrollbar_state = ScrollbarState::new(self.messages.len())
            .position(self.scroll_offset);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(colors::SURFACE2))
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        f.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input_text = match self.input_mode {
            InputMode::Editing => {
                Line::from(vec![
                    Span::styled("❯ ", Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD)),
                    Span::styled(&self.input, Style::default().fg(colors::TEXT)),
                    Span::styled("█", Style::default().fg(colors::LAVENDER)),
                ])
            }
            InputMode::Normal => {
                Line::from(vec![
                    Span::styled("  ", Style::default().fg(colors::OVERLAY0)),
                    Span::styled("Press 'i' to type a message...", Style::default().fg(colors::OVERLAY1).add_modifier(Modifier::ITALIC)),
                ])
            }
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if self.input_mode == InputMode::Editing {
                colors::BLUE
            } else {
                colors::SURFACE1
            }))
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    if self.input_mode == InputMode::Editing { "Input" } else { "Ready" },
                    Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)
                ),
                Span::raw(" "),
            ]))
            .style(Style::default().bg(colors::MANTLE));

        let paragraph = Paragraph::new(input_text)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let status_spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled(&self.status_line, Style::default().fg(colors::TEXT)),
            Span::raw(" "),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::SURFACE1))
            .style(Style::default().bg(colors::CRUST));

        let paragraph = Paragraph::new(Line::from(status_spans))
            .block(block);

        f.render_widget(paragraph, area);
    }

    fn render_help_overlay(&self, f: &mut Frame, area: Rect) {
        let help_area = centered_rect(60, 50, area);

        let help_text = vec![
            Line::from(vec![
                Span::styled("  Help  ", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Modes:", Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("Normal", Style::default().fg(colors::YELLOW)),
                Span::raw(" - Navigate and control"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("Insert", Style::default().fg(colors::GREEN)),
                Span::raw(" - Type messages"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Keybindings:", Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("i", Style::default().fg(colors::MAUVE)),
                Span::raw("          - Enter insert mode"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("Esc", Style::default().fg(colors::MAUVE)),
                Span::raw("        - Return to normal mode"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("Enter", Style::default().fg(colors::MAUVE)),
                Span::raw("      - Send message (insert mode)"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("↑/↓", Style::default().fg(colors::MAUVE)),
                Span::raw("        - Scroll messages"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("PgUp/PgDn", Style::default().fg(colors::MAUVE)),
                Span::raw("   - Fast scroll"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("?", Style::default().fg(colors::MAUVE)),
                Span::raw("          - Toggle this help"),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("q", Style::default().fg(colors::MAUVE)),
                Span::raw("          - Quit"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press '?' to close", Style::default().fg(colors::OVERLAY1).add_modifier(Modifier::ITALIC)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::LAVENDER))
            .style(Style::default().bg(colors::BASE))
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled("❓ Help", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
            ]));

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, help_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn run_tui_session(_messages: Vec<Message>) -> Result<mpsc::UnboundedSender<UiEvent>> {
    // This function is deprecated, kept for compatibility
    let (_app, tx) = TuiApp::new();
    Ok(tx)
}

