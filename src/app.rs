use std::error::Error;
use std::sync::{
    mpsc::Receiver,
    Arc,
    Mutex,
};
use std::{env, path::PathBuf, thread, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use log::error;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::Style,
    widgets::Block,
    Frame,
};
use rodio::OutputStream;

use crate::{
    config::Config,
    file_browser::Browser,
    structs::song::directory_to_songs_and_folders,
    player::Player,
    state::State,
    term::set_terminal, ui,
    Command,
};
use crate::ui::stateful_list::StatefulList;
use crate::ui::stateful_table::StatefulTable;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum InputMode {
    Browser,
    Queue,
    HelpControls,
}

#[derive(Debug, Clone, Copy)]
pub enum AppTab {
    FileBrowser = 0,
    Help,
}

pub struct App<'a> {
    must_quit: bool,
    input_mode: InputMode,
    active_tab: AppTab,
    browser: Browser,
    control_table: StatefulTable<'a>,
    player_command_receiver: Arc<Mutex<Receiver<Command>>>,
    player: Arc<Player>,
    #[allow(dead_code)]
    music_output: OutputStream,
}

impl<'a> App<'a> {
    pub fn new(
        initial_directory: Option<String>,
        queue: Vec<String>,
        player_command_receiver: Receiver<Command>,
    ) -> Self {
        let current_directory = match &initial_directory {
            Some(s) => PathBuf::from(s),
            None => env::current_dir().unwrap(),
        };

        let mut browser_items = StatefulList::with_items(directory_to_songs_and_folders(&current_directory));
        browser_items.select(0);

        let music_output = OutputStream::try_default().unwrap();
        // music_output.0 can be neither dropped nor shared between threads.
        // The underlying library is not thread-safe.
        // We could do this to prevent it from ever being dropped, but it's overkill and bug-prone.
        // std::mem::forget(music_output.0);

        Self {
            must_quit: false,
            input_mode: InputMode::Browser,
            active_tab: AppTab::FileBrowser,
            browser: Browser::new(browser_items, current_directory),
            control_table: StatefulTable::new(),
            player_command_receiver: Arc::new(Mutex::new(player_command_receiver)),
            player: Arc::new(Player::new(queue, &music_output.1)),
            music_output: music_output.0,
        }
    }

    fn to_state(&self) -> State {
        let player = self.player.clone();

        let queue_items = player
            .queue()
            .paths()
            .iter()
            .filter_map(|i| i.to_str())
            .map(|i| i.to_string())
            .collect();

        State {
            last_visited_path: self.browser.current_directory.to_str().map(String::from),
            queue_items,
        }
    }

    pub fn start(&mut self) -> Result<State, Box<dyn Error>> {
        let mut terminal = set_terminal()?;

        let tick_rate = Duration::from_secs(1);
        let mut last_tick = std::time::Instant::now();

        self.spawn_media_key_receiver_thread();
        self.player.spawn_player_thread();

        while !self.must_quit {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key);
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = std::time::Instant::now();
            }
        }

        Ok(self.to_state())
    }

    fn spawn_media_key_receiver_thread(&self) {
        let player_command_receiver = self.player_command_receiver.clone();
        let player = self.player.clone();

        thread::spawn(move || loop {
            match player_command_receiver.lock().unwrap().recv() {
                Ok(Command::PlayPause) => {
                    player.toggle();
                }
                Ok(Command::Next) => {
                    player.stop();
                }
                Err(err) => {
                    error!("error receiving! {}", err);
                    break;
                }
            }
        });
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        let focus_trapped = self.input_mode == InputMode::Browser && self.browser.filter.is_some();
        let handled = !focus_trapped && self.handle_app_key_event(&key);

        if !handled {
            match self.input_mode {
                InputMode::Browser => self.handle_browser_key_events(key),
                InputMode::Queue => self.handle_queue_key_events(key),
                InputMode::HelpControls => self.handle_help_key_events(key),
            }
        }
    }

    fn handle_app_key_event(&mut self, key: &KeyEvent) -> bool {
        let mut handled = true;
        match key.code {
            KeyCode::Char('q') => {
                self.must_quit = true;
            }
            KeyCode::Char('1') => {
                self.active_tab = AppTab::FileBrowser;
                self.input_mode = InputMode::Browser;
            }
            KeyCode::Char('2') => {
                self.active_tab = AppTab::Help;
                self.input_mode = InputMode::HelpControls;
            }
            KeyCode::Tab if self.browser.filter.is_none() => {
                match self.active_tab {
                    AppTab::FileBrowser => {
                        self.input_mode = match self.input_mode {
                            InputMode::Browser => InputMode::Queue,
                            InputMode::Queue => InputMode::Browser,
                            e => e,
                        };

                        // TODO: focus/blur colors
                        match self.input_mode {
                            InputMode::Browser => {
                                self.browser.items.next();
                                self.player.queue().select_none();
                            }
                            InputMode::Queue => {
                                self.browser.items.unselect();
                                self.player.queue().select_next();
                            }
                            _ => {}
                        };
                    }
                    _ => {}
                }
            }
            KeyCode::Right => self.player.seek_forward(),
            KeyCode::Left => self.player.seek_backward(),
            KeyCode::Char('-') => self.player.change_volume(-0.05),
            KeyCode::Char('+') => self.player.change_volume(0.05),
            KeyCode::Char('p') | KeyCode::Char(' ') => self.player.toggle(),
            KeyCode::Char('g') => self.player.stop(),
            _ => {
                handled = false;
            }
        }
        handled
    }

    fn handle_browser_key_events(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter if key.modifiers == KeyModifiers::ALT => {
                self.player.queue().add(self.browser.selected_item());
                self.browser.items.next();
            }
            KeyCode::Char('a') if self.browser.filter.is_none() => {
                self.player.queue().add(self.browser.selected_item());
                self.browser.items.next();
            }
            KeyCode::Enter => {
                if let Some(song) = self.browser.enter_selection() {
                    self.player.play_now(song);
                }
            }
            _ => {}
        }

        self.browser.on_key_event(key);
    }

    fn handle_queue_key_events(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Some(song) = self.player.queue().selected_song() {
                    self.player.play_now(song);
                };
            }
            KeyCode::Down | KeyCode::Char('j') => self.player.queue().select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.player.queue().select_previous(),
            KeyCode::Delete => self.player.queue().remove_selected(),
            _ => {}
        }
    }

    fn handle_help_key_events(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.control_table.next(),
            KeyCode::Up | KeyCode::Char('k') => self.control_table.previous(),
            _ => {}
        }
    }

    fn render(self: &mut Self, frame: &mut Frame) {
        let config = Config::new();
        let area = frame.size();

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0), Constraint::Length(3)].as_ref())
            .split(area);

        let block = Block::default().style(Style::default().bg(config.background()));
        frame.render_widget(block, area);

        ui::render_ui::render_top_bar(frame, &config, areas[0], self.active_tab);

        match self.active_tab {
            AppTab::FileBrowser => self.browser.render(frame, &self.player.queue(), areas[1], &config),
            AppTab::Help => ui::instructions_tab::instructions_tab(frame, &mut self.control_table, areas[1], &config),
        };

        let currently_playing = self.player.currently_playing();
        let currently_playing = currently_playing.lock().unwrap();

        ui::render_ui::render_playing_gauge(
            frame,
            &config,
            areas[2],
            &currently_playing,
            self.player.get_pos(),
            self.player.queue().total_time(),
            self.player.queue().length(),
        );
    }
}
