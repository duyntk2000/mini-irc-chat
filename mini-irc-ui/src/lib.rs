mod widgets;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::{
    collections::BTreeSet,
    io::{self, Stdout},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols::DOT,
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};
use widgets::Input;

pub type MyTerminal = Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Copy, Clone, PartialEq)]
enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, Default)]
pub(crate) struct Tab {
    name: String,
    history: Vec<(String, String)>,
    offset: usize,
    users: BTreeSet<String>,
    /// Current value of the input box
    input: Input,
    has_unread_message: bool,
}

impl Tab {
    pub(crate) fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
}

/// App holds the state of the application
#[derive(Default)]
pub struct App {
    state: AppState,
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
    // input_width: u16,  TODO: find the input width is useful
}

pub struct AppState {
    /// Current input mode
    input_mode: InputMode,
    /// Tabs: one for every chan joined and private conversation
    tabs: Vec<Tab>,
    /// Notification to display.
    notif: Option<String>,
    /// Index of the current tab.
    current_tab: Option<usize>,
    /// Empty tab.
    empty_tab: Box<Tab>,
}

impl Default for AppState {
    fn default() -> AppState {
        AppState {
            input_mode: InputMode::Normal,
            tabs: Vec::new(),
            notif: None,
            current_tab: None,
            empty_tab: Box::new(Tab::default()),
        }
    }
}

impl App {
    pub fn start(&mut self) -> io::Result<()> {
        self.terminal = Some(start_ui()?);
        Ok(())
    }
    pub fn draw(&mut self) -> io::Result<()> {
        self.terminal.as_mut().expect("App::draw() can only be called after a successful call to App::start(), and cannot be called after an errorring call to App::draw()")
        .draw(|f| ui(f, &mut self.state)).map(|_| ())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.terminal.iter_mut().for_each(|terminal| {
            let _ = stop_ui(terminal); // we can only ignore the error - it's now too late to react
        })
    }
}

impl AppState {
    pub(crate) fn get_mut_current_tab(&mut self) -> &mut Tab {
        if !self.tabs.is_empty() && self.current_tab.is_some() {
            self.tabs.get_mut(self.current_tab.unwrap()).unwrap()
        } else {
            &mut self.empty_tab
        }
    }

    pub(crate) fn get_mut_tab_or_insert(&mut self, tab: String) -> &mut Tab {
        if let Some(index) = self.get_tab_index(&tab) {
            self.tabs.get_mut(index)
        } else {
            self.tabs.push(Tab::new(tab));
            self.tabs.last_mut()
        }
        .unwrap()
    }

    pub fn get_tab_index(&self, tab: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.name == tab)
    }

    pub fn is_current_tab(&self, index: usize) -> bool {
        if let Some(current_index) = self.current_tab {
            current_index == index
        } else {
            false
        }
    }

    pub fn unset_unread_message(&mut self) {
        self.get_mut_current_tab().has_unread_message = false;
    }

    pub fn current_users(&self) -> Option<impl Iterator<Item = &String>> {
        if !self.tabs.is_empty() && self.current_tab.is_some() {
            Some(
                self.tabs
                    .get(self.current_tab.unwrap())
                    .unwrap()
                    .users
                    .iter(),
            )
        } else {
            None
        }
    }
}

pub fn start_ui() -> io::Result<MyTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn stop_ui(terminal: &mut MyTerminal) -> io::Result<()> {
    // restore terminal
    disable_raw_mode()?;
    terminal
        .backend_mut()
        .execute(LeaveAlternateScreen)?
        .execute(DisableMouseCapture)?;
    terminal.show_cursor()
}

pub enum KeyReaction {
    UserInput(String),
    Quit,
}

impl App {
    pub fn react_to_event(&mut self, event: Event) -> Option<KeyReaction> {
        // Mode-indepent actions
        let input_mode = self.state.input_mode;
        let tab = self.state.get_mut_current_tab();

        if let Event::Mouse(mouse_event) = event {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    tab.offset = std::cmp::min(tab.history.len(), tab.offset + 1);
                }

                MouseEventKind::ScrollDown => {
                    tab.offset = tab.offset.saturating_sub(1);
                    if tab.offset == 0 {
                        tab.has_unread_message = false;
                    }
                }

                _ => {}
            }
        }

        match input_mode {
            InputMode::Normal => {
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Char('e') => {
                            self.state.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            return Some(KeyReaction::Quit);
                        }
                        KeyCode::Left
                            if self.state.current_tab.is_some() && !self.state.tabs.is_empty() =>
                        {
                            let index = self.state.current_tab.unwrap();
                            self.state.current_tab = if index == 0 {
                                Some(self.state.tabs.len() - 1)
                            } else {
                                Some(index - 1)
                            };
                            self.state.unset_unread_message();
                        }
                        KeyCode::Right => {
                            if self.state.current_tab.is_some() && !self.state.tabs.is_empty() {
                                let index = self.state.current_tab.unwrap();
                                self.state.current_tab = if index == self.state.tabs.len() - 1 {
                                    Some(0)
                                } else {
                                    Some(index + 1)
                                };
                                self.state.unset_unread_message();
                            }
                        }
                        _ => {}
                    }
                }
            }

            InputMode::Editing => {
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Enter => {
                            let s = tab.input.submit();
                            let res = KeyReaction::UserInput(s);
                            return Some(res);
                        }

                        KeyCode::Char(c) => {
                            //Find the first character for which the cumulated width is larger than current offset
                            tab.input.insert_at_cursor(c);
                        }
                        KeyCode::Backspace => {
                            tab.input.delete_behind_cursor();
                        }

                        KeyCode::Delete => {
                            tab.input.delete_at_cursor();
                        }
                        KeyCode::Esc => {
                            self.state.input_mode = InputMode::Normal;
                        }
                        KeyCode::Left => {
                            tab.input.cursor_move_left();
                        }
                        KeyCode::Right => {
                            tab.input.cursor_move_right();
                        }
                        _ => {}
                    }
                }
            }
        }

        None
    }

    pub fn add_user(&mut self, username: String, tab: String) {
        let tab = self.state.get_mut_tab_or_insert(tab);
        tab.users.insert(username);
    }

    pub fn remove_user(&mut self, username: &str, tab: String) {
        if let Some(index) = self.state.get_tab_index(&tab) {
            let tab = self.state.tabs.get_mut(index).unwrap();
            tab.users.remove(username);
        }
    }

    pub fn add_tab(&mut self, tab: String) {
        if self.state.get_tab_index(&tab).is_none() {
            self.state.tabs.push(Tab::new(tab));
        }

        if self.state.current_tab.is_none() {
            self.state.current_tab = Some(0);
        }
    }

    pub fn add_tab_with_users(&mut self, tab: String, users: Vec<String>) {
        if self.state.get_tab_index(&tab).is_none() {
            let mut tab = Tab::new(tab);
            users.into_iter().for_each(|nickname| {
                tab.users.insert(nickname);
            });
            self.state.tabs.push(tab);
        }

        if self.state.current_tab.is_none() {
            self.state.current_tab = Some(0);
        }
    }

    /// Remove a tab.
    pub fn remove_tab(&mut self, tab: String) {
        if let (Some(index), Some(current_index)) =
            (self.state.get_tab_index(&tab), self.state.current_tab)
        {
            let _ = self.state.tabs.remove(index);
            if index <= current_index && index > 0 {
                self.state.current_tab = Some(current_index - 1);
            } else if self.state.tabs.is_empty() {
                self.state.current_tab = None
            }
        }
    }

    pub fn push_message(&mut self, from: String, message: String, tab_name: String) {
        if let Some(index) = self.state.get_tab_index(&tab_name) {
            // Tab exists for sure here.
            let is_current_tab = self.state.is_current_tab(index);
            let tab = self.state.get_mut_tab_or_insert(tab_name.clone());
            tab.history.push((from, message));
            if tab.offset != 0 || !is_current_tab {
                tab.has_unread_message = true;
            }
        }
    }

    pub fn get_current_tab(&self) -> String {
        if !self.state.tabs.is_empty() && self.state.current_tab.is_some() {
            self.state
                .tabs
                .get(self.state.current_tab.unwrap())
                .unwrap()
        } else {
            &self.state.empty_tab
        }
        .name
        .clone()
    }

    /// Set a new notification to print.
    /// Might erase an old one.
    pub fn set_notification(&mut self, notif: String) {
        self.state.notif = Some(notif);
    }

    /// Clear the current notification.
    pub fn clear_notif(&mut self) {
        self.state.notif.take();
    }
}

pub fn ui<B: Backend>(f: &mut Frame<B>, app_state: &mut AppState) {
    let input_mode = app_state.input_mode;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.size());

    let (msg, style) = match app_state.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("e", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to enter messages."),
            ],
            Style::default(),
            //Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send the message"),
            ],
            Style::default(),
        ),
    };
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[1]);

    // Channel list
    if app_state.tabs.is_empty() {
        f.render_widget(
            Paragraph::new("Waiting for connexion...").block(
                Block::default()
                    .title("Conversations")
                    .borders(Borders::ALL),
            ),
            chunks[3],
        )
    } else {
        let titles = app_state
            .tabs
            .iter()
            .map(|tab| {
                if tab.has_unread_message {
                    Span::styled(
                        tab.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::from(tab.name.clone())
                }
            })
            .map(Spans::from)
            .collect();
        // TODO add bold style for tabs with unread messages
        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title("Conversations")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .divider(DOT)
            .select(app_state.current_tab.unwrap_or_default());

        f.render_widget(tabs, chunks[3]);
    }

    let messages = app_state.get_mut_current_tab();

    messages.input.resize(chunks[2].width - 2);
    let input = Paragraph::new(messages.input.get_display_string())
        .style(match input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .block(Block::default().borders(Borders::ALL).title("Input"));

    f.render_widget(input, chunks[2]);

    match input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[2].x + messages.input.get_cursor_offset() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[2].y + 1,
            )
        }
    }

    let main_windows = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(15)].as_ref())
        .split(chunks[0]);

    let max_messages = (main_windows[0].height - 2) as usize;
    let to_skip = if messages.history.len() <= max_messages {
        0
    } else {
        messages.offset = std::cmp::min(messages.offset, messages.history.len() - max_messages);
        (messages.history.len() - max_messages).saturating_sub(messages.offset)
    };

    let messages: Vec<ListItem> = messages
        .history
        .iter()
        .skip(to_skip)
        .map(|m| {
            let content = vec![Spans::from(Span::raw(format!("{}: {}", m.0, m.1)))];
            ListItem::new(content)
        })
        .collect();
    let mut all_messages = vec![ListItem::new(" "); max_messages.saturating_sub(messages.len())];
    all_messages.extend(messages);
    let messages =
        List::new(all_messages).block(Block::default().borders(Borders::ALL).title("Messages"));

    f.render_widget(messages, main_windows[0]);

    let users = if let Some(users) = app_state.current_users() {
        List::new(
            users
                .map(|s| ListItem::new(s.to_string()))
                .collect::<Vec<_>>(),
        )
    } else {
        List::new(vec![ListItem::new("".to_string())])
    }
    .block(Block::default().borders(Borders::ALL).title("Connected"));
    f.render_widget(users, main_windows[1]);

    // Zone de notification pour les messages d'erreur
    let notif = app_state.notif.as_deref().unwrap_or_default();

    let notif = Paragraph::new(Text::from(notif)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Notifications"),
    );
    f.render_widget(notif, chunks[4]);
    //f.render_widget(messages, main_windows[1]);

    // f.render_widget(main_windows, chunks[0]);
}
