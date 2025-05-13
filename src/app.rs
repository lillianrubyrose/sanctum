use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use crossterm::event::KeyEventKind;
use ratatui::{
	DefaultTerminal, Frame,
	crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
	widgets::{ScrollbarState, TableState},
};
use rusqlite::Connection;
use std::sync::Arc;
use tui_textarea::TextArea;
use unicode_width::UnicodeWidthStr;

use crate::db::{self, PartialJournalEntry, PartialMoodEntry};
use crate::view;

pub type AppResult<T> = Result<T, AppError>;
type OnConfirmationFn = fn(&mut App) -> AppResult<()>;

#[derive(Debug)]
pub enum AppError {
	Io(std::io::Error),
	Sqlite(rusqlite::Error),
	SqliteMigration(rusqlite_migration::Error),
	Encryption(String),
}

pub struct App<'a> {
	conn: Connection,
	cipher: XChaCha20Poly1305,
	mood_entries: Arc<Vec<PartialMoodEntry>>,
	journal_entries: Arc<Vec<PartialJournalEntry>>,
	pub running: bool,
	pub view: AppView<'a>,
	pub next_view: Option<AppView<'a>>,
}

impl<'a> App<'a> {
	pub fn switch_view(&mut self, new_view: AppView<'a>) {
		self.view = new_view;
	}

	pub fn require_confirmation(&mut self, prompt: &'static str, confirm_fn: OnConfirmationFn) {
		let mut previous_view = AppView::MainMenu;
		std::mem::swap(&mut self.view, &mut previous_view);

		self.view = AppView::Confirmation {
			prompt,
			previous_view: Box::new(previous_view),
			confirm_fn,
			selected_button: ConfirmationButton::No,
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MoodEntryViewField {
	Mood,
	Journal,
	TimeOffset,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JournalEntryViewField {
	Journal,
	TimeOffset,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfirmationButton {
	Yes,
	No,
}

#[derive(Clone)]
pub enum AppView<'a> {
	MainMenu,
	NewMoodEntry {
		mood_rating: i8,
		time_offset_hours: i8,
		focused_field: MoodEntryViewField,
		journal_textarea: TextArea<'a>,
	},
	NewJournalEntry {
		time_offset_hours: i8,
		focused_field: JournalEntryViewField,
		journal_textarea: TextArea<'a>,
	},
	ViewMoodEntry {
		mood_rating: String,
		formatted_date: String,
		journal_text: String,
		scroll_state: ScrollbarState,
		scroll_position: usize,
	},
	ViewJournalEntry {
		formatted_date: String,
		journal_text: String,
		scroll_state: ScrollbarState,
		scroll_position: usize,
	},
	Confirmation {
		prompt: &'static str,
		selected_button: ConfirmationButton,
		previous_view: Box<AppView<'a>>,
		confirm_fn: OnConfirmationFn,
	},
	MoodEntryHistory {
		entries: Arc<Vec<PartialMoodEntry>>,
		table_state: TableState,
		scroll_state: ScrollbarState,
		largest_entries: (u16, u16),
	},
	JournalEntryHistory {
		entries: Arc<Vec<PartialJournalEntry>>,
		table_state: TableState,
		scroll_state: ScrollbarState,
		largest_entry_date: u16,
	},
	EmptyMoodEntryHistory,
	EmptyJournalEntryHistory,
	Onboarding {
		passphrase_textarea: TextArea<'a>,
		error: Option<&'static str>,
	},
	OnboardingConfirmPassphrase {
		initial_passphrase: String,
		passphrase_textarea: TextArea<'a>,
		error: Option<&'static str>,
	},
	Login {
		passphrase_textarea: TextArea<'a>,
		error: Option<&'static str>,
	},
}

impl AppView<'_> {
	const MOOD_ENTRY_HEIGHT: usize = 4;
	const JOURNAL_ENTRY_HEIGHT: usize = 3;

	pub fn new_mood_entry() -> Self {
		Self::NewMoodEntry {
			mood_rating: 0,
			time_offset_hours: 0,
			focused_field: MoodEntryViewField::Mood,
			journal_textarea: TextArea::default(),
		}
	}

	pub fn mood_entry_history(entries: Arc<Vec<PartialMoodEntry>>) -> Self {
		fn constraint_len_calculator(entries: &[PartialMoodEntry]) -> (u16, u16) {
			let mood_rating_len = entries
				.iter()
				.map(PartialMoodEntry::mood_rating_str)
				.map(|str| UnicodeWidthStr::width(str.as_str()))
				.max()
				.unwrap_or(0);
			let date_len = entries
				.iter()
				.map(PartialMoodEntry::formatted_date)
				.flat_map(str::lines)
				.map(UnicodeWidthStr::width)
				.max()
				.unwrap_or(0);

			#[allow(clippy::cast_possible_truncation)]
			(date_len as u16, mood_rating_len as u16)
		}

		AppView::MoodEntryHistory {
			table_state: TableState::new().with_selected(0),
			scroll_state: ScrollbarState::new((entries.len().saturating_sub(1)) * AppView::MOOD_ENTRY_HEIGHT),
			largest_entries: constraint_len_calculator(&entries),
			entries,
		}
	}

	pub fn view_mood_entry(mood_entry: PartialMoodEntry, journal_text: String) -> Self {
		let line_count = journal_text.lines().count();
		Self::ViewMoodEntry {
			mood_rating: mood_entry.mood_rating_str(),
			formatted_date: mood_entry.formatted_date,
			journal_text,
			scroll_state: ScrollbarState::new(line_count.saturating_sub(1)),
			scroll_position: 0,
		}
	}

	pub fn new_journal_entry() -> Self {
		Self::NewJournalEntry {
			time_offset_hours: 0,
			focused_field: JournalEntryViewField::Journal,
			journal_textarea: TextArea::default(),
		}
	}

	pub fn journal_entry_history(entries: Arc<Vec<PartialJournalEntry>>) -> Self {
		fn constraint_len_calculator(entries: &[PartialJournalEntry]) -> u16 {
			let date_len = entries
				.iter()
				.map(PartialJournalEntry::formatted_date)
				.flat_map(str::lines)
				.map(UnicodeWidthStr::width)
				.max()
				.unwrap_or(0);

			#[allow(clippy::cast_possible_truncation)]
			(date_len as u16)
		}

		AppView::JournalEntryHistory {
			table_state: TableState::new().with_selected(0),
			scroll_state: ScrollbarState::new((entries.len().saturating_sub(1)) * AppView::JOURNAL_ENTRY_HEIGHT),
			largest_entry_date: constraint_len_calculator(&entries),
			entries,
		}
	}

	pub fn view_journal_entry(journal_entry: PartialJournalEntry, journal_text: String) -> Self {
		let line_count = journal_text.lines().count();
		Self::ViewJournalEntry {
			formatted_date: journal_entry.formatted_date,
			journal_text,
			scroll_state: ScrollbarState::new(line_count.saturating_sub(1)),
			scroll_position: 0,
		}
	}

	pub fn onboarding() -> Self {
		let mut passphrase_textarea = TextArea::default();
		passphrase_textarea.set_mask_char('*');
		passphrase_textarea.set_max_histories(0);
		Self::Onboarding {
			passphrase_textarea,
			error: None,
		}
	}

	pub fn onboarding_confirm(initial_passphrase: String) -> Self {
		let mut passphrase_textarea = TextArea::default();
		passphrase_textarea.set_mask_char('*');
		passphrase_textarea.set_max_histories(0);
		Self::OnboardingConfirmPassphrase {
			initial_passphrase,
			passphrase_textarea,
			error: None,
		}
	}

	pub fn login() -> Self {
		let mut passphrase_textarea = TextArea::default();
		passphrase_textarea.set_mask_char('*');
		passphrase_textarea.set_max_histories(0);
		Self::Login {
			passphrase_textarea,
			error: None,
		}
	}
}

impl App<'_> {
	pub fn new(conn: Connection) -> Self {
		Self {
			conn,
			cipher: XChaCha20Poly1305::new(&[0u8; 32].into()),
			mood_entries: Arc::new(Vec::new()),
			journal_entries: Arc::new(Vec::new()),
			running: true,
			view: AppView::login(),
			next_view: None,
		}
	}

	pub fn run(mut self, mut terminal: DefaultTerminal) -> AppResult<()> {
		if db::needs_first_setup(&mut self.conn)? {
			self.view = AppView::onboarding();
		}

		while self.running {
			terminal.draw(|frame| self.render(frame)).map_err(AppError::Io)?;
			self.handle_events()?;
		}
		Ok(())
	}

	pub fn render(&mut self, frame: &mut Frame) {
		view::render_app(self, frame);
	}

	pub fn handle_events(&mut self) -> AppResult<()> {
		match crossterm::event::read() {
			Ok(event) => {
				if let crossterm::event::Event::Key(key_event) = event {
					self.handle_key_event(key_event)?
				}

				Ok(())
			}
			Err(err) => Err(AppError::Io(err)),
		}
	}

	pub fn handle_key_event(&mut self, key_event: KeyEvent) -> AppResult<()> {
		if key_event.kind == KeyEventKind::Release {
			return Ok(());
		}

		match &mut self.view {
			AppView::MainMenu => match key_event.code {
				KeyCode::Char('1') => self.switch_view(AppView::new_mood_entry()),
				KeyCode::Char('2') => self.switch_view(AppView::new_journal_entry()),
				KeyCode::Char('3') => {
					if self.mood_entries.is_empty() {
						self.switch_view(AppView::EmptyMoodEntryHistory);
					} else {
						self.switch_view(AppView::mood_entry_history(Arc::clone(&self.mood_entries)));
					}
				}
				KeyCode::Char('4') => {
					if self.journal_entries.is_empty() {
						self.switch_view(AppView::EmptyJournalEntryHistory);
					} else {
						self.switch_view(AppView::journal_entry_history(Arc::clone(&self.journal_entries)));
					}
				}
				KeyCode::Esc | KeyCode::Char('q') => self.quit(),
				KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
					self.quit();
				}
				_ => {}
			},
			AppView::NewMoodEntry {
				mood_rating,
				focused_field,
				time_offset_hours,
				journal_textarea,
			} => match key_event.code {
				KeyCode::Esc => self.require_confirmation("Are you sure you want to exit without saving?", |app| {
					app.switch_view(AppView::MainMenu);
					Ok(())
				}),
				KeyCode::Tab => {
					*focused_field = match *focused_field {
						MoodEntryViewField::Mood => MoodEntryViewField::Journal,
						MoodEntryViewField::Journal => MoodEntryViewField::TimeOffset,
						MoodEntryViewField::TimeOffset => MoodEntryViewField::Mood,
					}
				}
				KeyCode::Up if *focused_field == MoodEntryViewField::Mood => {
					*mood_rating = (*mood_rating + 1).min(10);
				}
				KeyCode::Down if *focused_field == MoodEntryViewField::Mood => {
					*mood_rating = (*mood_rating - 1).max(-10);
				}
				KeyCode::Up if *focused_field == MoodEntryViewField::TimeOffset => {
					*time_offset_hours = (*time_offset_hours + 1).min(0);
				}
				KeyCode::Down if *focused_field == MoodEntryViewField::TimeOffset => {
					*time_offset_hours = (*time_offset_hours - 1).max(-24);
				}
				KeyCode::Enter if *focused_field != MoodEntryViewField::Journal => {
					self.next_view = Some(AppView::NewMoodEntry {
						mood_rating: *mood_rating,
						time_offset_hours: *time_offset_hours,
						focused_field: *focused_field,
						journal_textarea: journal_textarea.clone(),
					});

					self.require_confirmation("Are you sure you want to save this mood entry?", |app| {
						if let Some(AppView::NewMoodEntry {
							mood_rating,
							time_offset_hours,
							mut journal_textarea,
							..
						}) = app.next_view.take()
						{
							journal_textarea.select_all();
							journal_textarea.copy();
							let journal_text = journal_textarea.yank_text();
							let entry = db::save_mood_entry(
								&mut app.conn,
								&app.cipher,
								mood_rating,
								journal_text.clone(),
								time_offset_hours,
							)?;

							let mut new_entries = Vec::with_capacity(app.mood_entries.len() + 1);
							new_entries.extend_from_slice(&app.mood_entries);
							new_entries.push(entry);
							new_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
							app.mood_entries = Arc::new(new_entries);

							app.switch_view(AppView::MainMenu);
						}
						Ok(())
					});
				}
				_ if *focused_field == MoodEntryViewField::Journal => {
					journal_textarea.input(crossterm::event::Event::Key(key_event));
				}
				_ => {}
			},
			AppView::NewJournalEntry {
				time_offset_hours,
				focused_field,
				journal_textarea,
			} => match key_event.code {
				KeyCode::Esc => self.require_confirmation("Are you sure you want to exit without saving?", |app| {
					app.switch_view(AppView::MainMenu);
					Ok(())
				}),
				KeyCode::Tab => {
					*focused_field = match *focused_field {
						JournalEntryViewField::Journal => JournalEntryViewField::TimeOffset,
						JournalEntryViewField::TimeOffset => JournalEntryViewField::Journal,
					}
				}
				KeyCode::Up if *focused_field == JournalEntryViewField::TimeOffset => {
					*time_offset_hours = (*time_offset_hours + 1).min(0);
				}
				KeyCode::Down if *focused_field == JournalEntryViewField::TimeOffset => {
					*time_offset_hours = (*time_offset_hours - 1).max(-24);
				}
				KeyCode::Enter if *focused_field != JournalEntryViewField::Journal => {
					self.next_view = Some(AppView::NewJournalEntry {
						time_offset_hours: *time_offset_hours,
						focused_field: *focused_field,
						journal_textarea: journal_textarea.clone(),
					});

					self.require_confirmation("Are you sure you want to save this journal entry?", |app| {
						if let Some(AppView::NewJournalEntry {
							time_offset_hours,
							mut journal_textarea,
							..
						}) = app.next_view.take()
						{
							journal_textarea.select_all();
							journal_textarea.copy();
							let journal_text = journal_textarea.yank_text();
							let entry = db::save_journal_entry(
								&mut app.conn,
								&app.cipher,
								journal_text.clone(),
								time_offset_hours,
							)?;

							let mut new_entries = Vec::with_capacity(app.journal_entries.len() + 1);
							new_entries.extend_from_slice(&app.journal_entries);
							new_entries.push(entry);
							new_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
							app.journal_entries = Arc::new(new_entries);

							app.switch_view(AppView::MainMenu);
						}
						Ok(())
					});
				}
				_ if *focused_field == JournalEntryViewField::Journal => {
					journal_textarea.input(crossterm::event::Event::Key(key_event));
				}
				_ => {}
			},
			AppView::Confirmation {
				prompt: _,
				selected_button,
				previous_view,
				confirm_fn,
			} => match key_event.code {
				KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
					let mut prev = AppView::MainMenu;
					std::mem::swap(&mut prev, previous_view.as_mut());
					self.switch_view(prev);
				}
				KeyCode::Char('y') | KeyCode::Char('Y') => {
					(*confirm_fn)(self)?;
				}
				KeyCode::Left | KeyCode::Right => {
					*selected_button = match *selected_button {
						ConfirmationButton::Yes => ConfirmationButton::No,
						ConfirmationButton::No => ConfirmationButton::Yes,
					}
				}
				KeyCode::Enter => match *selected_button {
					ConfirmationButton::Yes => {
						(*confirm_fn)(self)?;
					}
					ConfirmationButton::No => {
						let mut prev = AppView::MainMenu;
						std::mem::swap(&mut prev, previous_view.as_mut());
						self.switch_view(prev);
					}
				},
				_ => {}
			},
			AppView::MoodEntryHistory {
				entries,
				table_state,
				scroll_state,
				largest_entries: _,
			} => match key_event.code {
				KeyCode::Esc => {
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Down => {
					let idx = table_state
						.selected()
						.map(|i| if i >= entries.len() - 1 { 0 } else { i + 1 })
						.unwrap_or(0);
					table_state.select(Some(idx));
					*scroll_state = scroll_state.position(idx * AppView::MOOD_ENTRY_HEIGHT);
				}
				KeyCode::Up => {
					let idx = table_state
						.selected()
						.map(|i| if i == 0 { entries.len() - 1 } else { i - 1 })
						.unwrap_or(0);
					table_state.select(Some(idx));
					*scroll_state = scroll_state.position(idx * AppView::MOOD_ENTRY_HEIGHT);
				}
				KeyCode::Enter => {
					let idx = table_state.selected().expect("row to be selected");
					let entry = &entries[idx];

					let journal_entry = db::get_journal_text(&mut self.conn, &self.cipher, &entry.id, true)?;

					self.next_view = Some(AppView::view_mood_entry(entry.clone(), journal_entry));
					self.require_confirmation("Are you sure you want to view this entry?", |app| {
						if let Some(view) = app.next_view.take() {
							app.switch_view(view);
						}
						Ok(())
					});
				}
				_ => {}
			},
			AppView::ViewMoodEntry {
				mood_rating: _,
				formatted_date: _,
				journal_text,
				scroll_state,
				scroll_position,
			} => match key_event.code {
				KeyCode::Esc => {
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Up => {
					if *scroll_position > 0 {
						*scroll_position -= 1;
						*scroll_state = scroll_state.position(*scroll_position);
					}
				}
				KeyCode::Down => {
					let line_count = journal_text.lines().count();
					if *scroll_position < line_count.saturating_sub(1) {
						*scroll_position += 1;
						*scroll_state = scroll_state.position(*scroll_position);
					}
				}
				_ => {}
			},
			AppView::EmptyMoodEntryHistory => {
				if key_event.code == KeyCode::Esc {
					self.switch_view(AppView::MainMenu);
				}
			}
			AppView::EmptyJournalEntryHistory => {
				if key_event.code == KeyCode::Esc {
					self.switch_view(AppView::MainMenu);
				}
			}
			AppView::JournalEntryHistory {
				entries,
				table_state,
				scroll_state,
				largest_entry_date: _,
			} => match key_event.code {
				KeyCode::Esc => {
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Down => {
					let idx = table_state
						.selected()
						.map(|i| if i >= entries.len() - 1 { 0 } else { i + 1 })
						.unwrap_or(0);
					table_state.select(Some(idx));
					*scroll_state = scroll_state.position(idx * AppView::JOURNAL_ENTRY_HEIGHT);
				}
				KeyCode::Up => {
					let idx = table_state
						.selected()
						.map(|i| if i == 0 { entries.len() - 1 } else { i - 1 })
						.unwrap_or(0);
					table_state.select(Some(idx));
					*scroll_state = scroll_state.position(idx * AppView::JOURNAL_ENTRY_HEIGHT);
				}
				KeyCode::Enter => {
					let idx = table_state.selected().expect("row to be selected");
					let entry = &entries[idx];

					let journal_text = db::get_journal_text(&mut self.conn, &self.cipher, &entry.id, false)?;

					self.next_view = Some(AppView::view_journal_entry(entry.clone(), journal_text));
					self.require_confirmation("Are you sure you want to view this entry?", |app| {
						if let Some(view) = app.next_view.take() {
							app.switch_view(view);
						}
						Ok(())
					});
				}
				_ => {}
			},
			AppView::ViewJournalEntry {
				formatted_date: _,
				journal_text,
				scroll_state,
				scroll_position,
			} => match key_event.code {
				KeyCode::Esc => {
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Up => {
					if *scroll_position > 0 {
						*scroll_position -= 1;
						*scroll_state = scroll_state.position(*scroll_position);
					}
				}
				KeyCode::Down => {
					let line_count = journal_text.lines().count();
					if *scroll_position < line_count.saturating_sub(1) {
						*scroll_position += 1;
						*scroll_state = scroll_state.position(*scroll_position);
					}
				}
				_ => {}
			},
			AppView::Onboarding {
				passphrase_textarea,
				error,
			} => match key_event.code {
				KeyCode::Enter => {
					passphrase_textarea.select_all();
					passphrase_textarea.copy();
					let passphrase = passphrase_textarea.yank_text();
					if passphrase.len() < 8 {
						*error = Some("Passphrase minimum length is 8");
						return Ok(());
					}

					if passphrase.len() > 128 {
						*error = Some("Passphrase maximum length is 128");
						return Ok(());
					}

					self.switch_view(AppView::onboarding_confirm(passphrase));
				}
				KeyCode::Esc => {
					self.quit();
				}
				KeyCode::Char(_) | KeyCode::Backspace => {
					passphrase_textarea.input(crossterm::event::Event::Key(key_event));
				}
				_ => {}
			},
			AppView::OnboardingConfirmPassphrase {
				initial_passphrase,
				passphrase_textarea,
				error,
			} => match key_event.code {
				KeyCode::Enter => {
					passphrase_textarea.select_all();
					passphrase_textarea.copy();
					let passphrase = passphrase_textarea.yank_text();
					if &passphrase != initial_passphrase {
						*error = Some("Input doesn't match");
						return Ok(());
					}
					self.cipher = db::set_passphrase_initial(&mut self.conn, &passphrase)?;
					self.mood_entries = Arc::new(db::get_all_mood_entries(&mut self.conn, &self.cipher)?);
					self.journal_entries = Arc::new(db::get_all_journal_entries(&mut self.conn, &self.cipher)?);
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Esc => {
					self.switch_view(AppView::onboarding());
				}
				KeyCode::Char(_) | KeyCode::Backspace => {
					passphrase_textarea.input(crossterm::event::Event::Key(key_event));
				}
				_ => {}
			},
			AppView::Login {
				passphrase_textarea,
				error,
			} => match key_event.code {
				KeyCode::Enter => {
					passphrase_textarea.select_all();
					passphrase_textarea.copy();
					let passphrase = passphrase_textarea.yank_text();

					let Some(cipher) = db::get_encryption_cipher(&mut self.conn, &passphrase)? else {
						*error = Some("Passphrase is invalid");
						return Ok(());
					};

					self.cipher = cipher;
					self.mood_entries = Arc::new(db::get_all_mood_entries(&mut self.conn, &self.cipher)?);
					self.journal_entries = Arc::new(db::get_all_journal_entries(&mut self.conn, &self.cipher)?);
					self.switch_view(AppView::MainMenu);
				}
				KeyCode::Esc => {
					self.quit();
				}
				KeyCode::Char(_) | KeyCode::Backspace => {
					passphrase_textarea.input(crossterm::event::Event::Key(key_event));
				}
				_ => {}
			},
		}

		Ok(())
	}

	pub fn quit(&mut self) {
		self.running = false;
	}
}
