use ratatui::{
	Frame,
	layout::{Alignment, Constraint, Layout, Margin},
	style::{Color, Style, Stylize},
	text::Text,
	widgets::{
		Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
		Table, TableState, Wrap,
	},
};
use std::ops::Deref;
use std::sync::Arc;
use tui_textarea::TextArea;

use crate::app::{App, AppView, ConfirmationButton, JournalEntryViewField, MoodEntryViewField};
use crate::db::{PartialJournalEntry, PartialMoodEntry};

pub fn render_app(app: &mut App, frame: &mut Frame) {
	match &mut app.view {
		AppView::MainMenu => render_main_menu(frame),
		AppView::NewMoodEntry {
			mood_rating,
			focused_field,
			time_offset_hours,
			journal_textarea,
		} => render_new_mood_entry(frame, *mood_rating, focused_field, *time_offset_hours, journal_textarea),
		AppView::NewJournalEntry {
			time_offset_hours,
			focused_field,
			journal_textarea,
		} => render_new_journal_entry(frame, focused_field, *time_offset_hours, journal_textarea),
		AppView::Confirmation {
			prompt,
			previous_view: _,
			confirm_fn: _,
			selected_button,
		} => render_confirmation(frame, prompt, *selected_button),
		AppView::MoodEntryHistory {
			entries,
			table_state,
			scroll_state,
			largest_entries,
		} => render_mood_entry_history(frame, entries, table_state, scroll_state, *largest_entries),
		AppView::JournalEntryHistory {
			entries,
			table_state,
			scroll_state,
			largest_entry_date,
		} => render_journal_entry_history(frame, entries, table_state, scroll_state, *largest_entry_date),
		AppView::EmptyMoodEntryHistory => render_empty_mood_entry_history(frame),
		AppView::EmptyJournalEntryHistory => render_empty_journal_entry_history(frame),
		AppView::ViewMoodEntry {
			mood_rating,
			formatted_date,
			journal_text,
			scroll_state,
			scroll_position,
		} => render_view_mood_entry(
			frame,
			mood_rating,
			formatted_date,
			journal_text,
			scroll_state,
			*scroll_position,
		),
		AppView::ViewJournalEntry {
			formatted_date,
			journal_text,
			scroll_state,
			scroll_position,
		} => render_view_journal_entry(frame, formatted_date, journal_text, scroll_state, *scroll_position),
		AppView::Onboarding {
			passphrase_textarea,
			error,
		} => render_onboarding(frame, passphrase_textarea, *error),
		AppView::OnboardingConfirmPassphrase {
			initial_passphrase: _,
			passphrase_textarea,
			error,
		} => render_onboarding_confirm_passphrase(frame, passphrase_textarea, *error),
		AppView::Login {
			passphrase_textarea,
			error,
		} => render_login(frame, passphrase_textarea, *error),
	}
}

fn render_main_menu(frame: &mut Frame) {
	let block = Block::bordered()
		.title("Sanctum")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let text = "How are you feeling today?\n\n\
		[1] New Mood Entry\n\
		[2] New Journal Entry\n\
		[3] View Mood Entry History\n\
		[4] View Journal Entry History\n\n\
		Press `Esc` or `q` to exit";

	let paragraph = Paragraph::new(text)
		.block(block)
		.fg(Color::White)
		.alignment(Alignment::Center);

	frame.render_widget(paragraph, frame.area());
}

fn render_new_mood_entry(
	frame: &mut Frame,
	mood_rating: i8,
	focused_field: &MoodEntryViewField,
	time_offset_hours: i8,
	journal_textarea: &mut TextArea,
) {
	let [mood_area, journal_area, time_offset_area, help_area] = Layout::vertical([
		Constraint::Length(3),
		Constraint::Min(3),
		Constraint::Length(3),
		Constraint::Length(1),
	])
	.areas(frame.area());

	let mood_block = Block::bordered()
		.title("Mood Rating")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(if *focused_field == MoodEntryViewField::Mood {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		});

	let journal_block = Block::bordered()
		.title("Mood Journal Entry")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(if *focused_field == MoodEntryViewField::Journal {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		});
	journal_textarea.set_block(journal_block);

	let time_offset_block = Block::bordered()
		.title("Time Offset (Hours)")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(if *focused_field == MoodEntryViewField::TimeOffset {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		});

	let mood_text = format!("(-10 to 10): {}", mood_rating);
	let mood_paragraph = Paragraph::new(mood_text).block(mood_block).alignment(Alignment::Center);

	let time_offset_text = format!("{} hours", time_offset_hours);
	let time_offset_paragraph = Paragraph::new(time_offset_text)
		.block(time_offset_block)
		.alignment(Alignment::Center);

	let help_text = "↑/↓: Adjust Value | Tab: Switch Fields | Enter: Save (with confirmation) | Esc: Cancel";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);

	frame.render_widget(mood_paragraph, mood_area);
	frame.render_widget(journal_textarea.deref(), journal_area);
	frame.render_widget(time_offset_paragraph, time_offset_area);
	frame.render_widget(help_widget, help_area);
}

fn render_new_journal_entry(
	frame: &mut Frame,
	focused_field: &JournalEntryViewField,
	time_offset_hours: i8,
	journal_textarea: &mut TextArea,
) {
	let [journal_area, time_offset_area, help_area] =
		Layout::vertical([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)]).areas(frame.area());

	let journal_block = Block::bordered()
		.title("Journal Entry")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(if *focused_field == JournalEntryViewField::Journal {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		});
	journal_textarea.set_block(journal_block);

	let time_offset_block = Block::bordered()
		.title("Time Offset (Hours)")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(if *focused_field == JournalEntryViewField::TimeOffset {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		});

	let time_offset_text = format!("{} hours", time_offset_hours);
	let time_offset_paragraph = Paragraph::new(time_offset_text)
		.block(time_offset_block)
		.alignment(Alignment::Center);

	let help_text = "↑/↓: Adjust Value | Tab: Switch Fields | Enter: Save (with confirmation) | Esc: Cancel";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);

	frame.render_widget(journal_textarea.deref(), journal_area);
	frame.render_widget(time_offset_paragraph, time_offset_area);
	frame.render_widget(help_widget, help_area);
}

fn render_confirmation(frame: &mut Frame, prompt: &'static str, selected_button: ConfirmationButton) {
	let [prompt_area, buttons_area] = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).areas(frame.area());

	let main_block = Block::bordered()
		.title("Confirm")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	frame.render_widget(main_block.clone(), frame.area());

	let paragraph = Paragraph::new(prompt).fg(Color::White).alignment(Alignment::Center);

	frame.render_widget(paragraph, prompt_area);

	let [yes_area, no_area] =
		Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(buttons_area);

	let yes_block = Block::bordered().border_type(BorderType::Rounded).border_style(
		if selected_button == ConfirmationButton::Yes {
			Style::default().fg(Color::LightCyan)
		} else {
			Style::default().fg(Color::DarkGray)
		},
	);

	let no_block =
		Block::bordered()
			.border_type(BorderType::Rounded)
			.border_style(if selected_button == ConfirmationButton::No {
				Style::default().fg(Color::LightCyan)
			} else {
				Style::default().fg(Color::DarkGray)
			});

	let yes_button = Paragraph::new("[Y]es")
		.style(if selected_button == ConfirmationButton::Yes {
			Style::default().fg(Color::White)
		} else {
			Style::default().fg(Color::DarkGray)
		})
		.block(yes_block)
		.alignment(Alignment::Center);

	let no_button = Paragraph::new("[N]o")
		.style(if selected_button == ConfirmationButton::No {
			Style::default().fg(Color::White)
		} else {
			Style::default().fg(Color::DarkGray)
		})
		.block(no_block)
		.alignment(Alignment::Center);

	frame.render_widget(yes_button, yes_area);
	frame.render_widget(no_button, no_area);
}

fn render_mood_entry_history(
	frame: &mut Frame,
	entries: &Arc<Vec<PartialMoodEntry>>,
	table_state: &mut TableState,
	scroll_state: &mut ScrollbarState,
	largest_entries: (u16, u16),
) {
	let block = Block::bordered()
		.title("Mood Entry History")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let [table_area, scroll_area] = Layout::horizontal([Constraint::Min(5), Constraint::Length(4)]).areas(frame.area());

	let header = ["Date", "Mood Rating"]
		.into_iter()
		.map(|text| Cell::from(text).style(Style::default().fg(Color::LightCyan)))
		.collect::<Row>()
		.style(Style::default())
		.height(1);

	let rows = entries.iter().map(|entry| {
		let cells = [
			Cell::new(Text::from(format!("\n{}\n", entry.formatted_date))),
			Cell::new(Text::from(format!("\n{}\n", entry.mood_rating))),
		];
		Row::new(cells).style(Style::default()).height(4)
	});
	let bar = " █ ";
	let t = Table::new(
		rows,
		[
			Constraint::Length(largest_entries.0 + 1),
			Constraint::Min(largest_entries.1 + 1),
		],
	)
	.header(header)
	.highlight_symbol(Text::from(vec!["".into(), bar.into(), bar.into(), "".into()]))
	.block(block)
	.row_highlight_style(Style::default().fg(Color::LightCyan))
	.highlight_spacing(HighlightSpacing::Always);
	frame.render_stateful_widget(t, table_area, table_state);
	frame.render_stateful_widget(
		Scrollbar::default()
			.orientation(ScrollbarOrientation::VerticalRight)
			.begin_symbol(None)
			.end_symbol(None)
			.style(Style::default().fg(Color::LightCyan)),
		scroll_area.inner(Margin {
			vertical: 1,
			horizontal: 1,
		}),
		scroll_state,
	);
}

fn render_journal_entry_history(
	frame: &mut Frame,
	entries: &Arc<Vec<PartialJournalEntry>>,
	table_state: &mut TableState,
	scroll_state: &mut ScrollbarState,
	largest_entry_date: u16,
) {
	let block = Block::bordered()
		.title("Journal Entry History")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let [table_area, scroll_area] = Layout::horizontal([Constraint::Min(5), Constraint::Length(4)]).areas(frame.area());

	let header = ["Date"]
		.into_iter()
		.map(|text| Cell::from(text).style(Style::default().fg(Color::LightCyan)))
		.collect::<Row>()
		.style(Style::default())
		.height(1);

	let rows = entries.iter().map(|entry| {
		let cells = [Cell::new(Text::from(format!("\n{}\n", entry.formatted_date)))];
		Row::new(cells).style(Style::default()).height(3)
	});
	let bar = " █ ";
	let t = Table::new(rows, [Constraint::Length(largest_entry_date + 1)])
		.header(header)
		.highlight_symbol(Text::from(vec!["".into(), bar.into(), "".into()]))
		.block(block)
		.row_highlight_style(Style::default().fg(Color::LightCyan))
		.highlight_spacing(HighlightSpacing::Always);
	frame.render_stateful_widget(t, table_area, table_state);
	frame.render_stateful_widget(
		Scrollbar::default()
			.orientation(ScrollbarOrientation::VerticalRight)
			.begin_symbol(None)
			.end_symbol(None)
			.style(Style::default().fg(Color::LightCyan)),
		scroll_area.inner(Margin {
			vertical: 1,
			horizontal: 1,
		}),
		scroll_state,
	);
}

fn render_empty_mood_entry_history(frame: &mut Frame) {
	let block = Block::bordered()
		.title("Mood Entry History")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let message = "No mood entries found.\n\nCreate a new mood entry from the main menu.";
	let paragraph = Paragraph::new(message)
		.block(block)
		.fg(Color::White)
		.alignment(Alignment::Center);

	frame.render_widget(paragraph, frame.area());
}

fn render_empty_journal_entry_history(frame: &mut Frame) {
	let block = Block::bordered()
		.title("Journal Entry History")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let message = "No journal entries found.\n\nCreate a new journal entry from the main menu.";
	let paragraph = Paragraph::new(message)
		.block(block)
		.fg(Color::White)
		.alignment(Alignment::Center);

	frame.render_widget(paragraph, frame.area());
}

fn render_view_mood_entry(
	frame: &mut Frame,
	mood_rating: &str,
	date: &str,
	journal_text: &str,
	scroll_state: &mut ScrollbarState,
	scroll_position: usize,
) {
	let [mood_area, journal_area, time_offset_area, help_area] = Layout::vertical([
		Constraint::Length(3),
		Constraint::Min(3),
		Constraint::Length(3),
		Constraint::Length(1),
	])
	.areas(frame.area());

	let mood_block = Block::bordered()
		.title("Mood Rating")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let journal_block = Block::bordered()
		.title("Mood Journal Entry")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let date_block = Block::bordered()
		.title("Date")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let mood_text = format!("(-10 to 10): {}", mood_rating);
	let mood_paragraph = Paragraph::new(mood_text).block(mood_block).alignment(Alignment::Center);

	let [journal_text_area, scrollbar_area] =
		Layout::horizontal([Constraint::Min(5), Constraint::Length(1)]).areas(journal_area);

	let lines: Vec<&str> = journal_text.lines().collect();
	let journal_height = journal_text_area.height.saturating_sub(2) as usize;

	let visible_lines = if lines.len() > journal_height {
		let end = (scroll_position + journal_height).min(lines.len());
		let visible = &lines[scroll_position..end];
		visible.join("\n")
	} else {
		journal_text.to_string()
	};

	let journal_paragraph = Paragraph::new(visible_lines)
		.block(journal_block)
		.alignment(Alignment::Left)
		.wrap(Wrap { trim: true });

	let date_paragraph = Paragraph::new(date).block(date_block).alignment(Alignment::Center);

	let help_text = "Esc: Back to Main Menu | ↑/↓: Scroll";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);

	frame.render_widget(mood_paragraph, mood_area);
	frame.render_widget(journal_paragraph, journal_text_area);
	frame.render_stateful_widget(
		Scrollbar::default()
			.orientation(ScrollbarOrientation::VerticalRight)
			.begin_symbol(None)
			.end_symbol(None)
			.style(Style::default().fg(Color::LightCyan)),
		scrollbar_area,
		scroll_state,
	);
	frame.render_widget(date_paragraph, time_offset_area);
	frame.render_widget(help_widget, help_area);
}

fn render_view_journal_entry(
	frame: &mut Frame,
	date: &str,
	journal_text: &str,
	scroll_state: &mut ScrollbarState,
	scroll_position: usize,
) {
	let [journal_area, date_area, help_area] =
		Layout::vertical([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)]).areas(frame.area());

	let journal_block = Block::bordered()
		.title("Journal Entry")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let date_block = Block::bordered()
		.title("Date")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	let [journal_text_area, scrollbar_area] =
		Layout::horizontal([Constraint::Min(5), Constraint::Length(1)]).areas(journal_area);

	let lines: Vec<&str> = journal_text.lines().collect();
	let journal_height = journal_text_area.height.saturating_sub(2) as usize;

	let visible_lines = if lines.len() > journal_height {
		let end = (scroll_position + journal_height).min(lines.len());
		let visible = &lines[scroll_position..end];
		visible.join("\n")
	} else {
		journal_text.to_string()
	};

	let journal_paragraph = Paragraph::new(visible_lines)
		.block(journal_block)
		.alignment(Alignment::Left)
		.wrap(Wrap { trim: true });

	let date_paragraph = Paragraph::new(date).block(date_block).alignment(Alignment::Center);

	let help_text = "Esc: Back to Main Menu | ↑/↓: Scroll";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);

	frame.render_widget(journal_paragraph, journal_text_area);
	frame.render_stateful_widget(
		Scrollbar::default()
			.orientation(ScrollbarOrientation::VerticalRight)
			.begin_symbol(None)
			.end_symbol(None)
			.style(Style::default().fg(Color::LightCyan)),
		scrollbar_area,
		scroll_state,
	);
	frame.render_widget(date_paragraph, date_area);
	frame.render_widget(help_widget, help_area);
}

fn render_onboarding(frame: &mut Frame, passphrase_textarea: &mut TextArea, error: Option<&'static str>) {
	let main_block = Block::bordered()
		.title("Onboarding")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	frame.render_widget(main_block.clone(), frame.area());

	let inner_area = main_block.inner(frame.area());

	let [passphrase_area, error_area, help_area] =
		Layout::vertical([Constraint::Length(3), Constraint::Length(1), Constraint::Length(1)]).areas(inner_area);

	let passphrase_block = Block::bordered()
		.title("Passphrase")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));
	passphrase_textarea.set_block(passphrase_block);

	frame.render_widget(passphrase_textarea.deref(), passphrase_area);

	if let Some(err_msg) = error {
		let error_paragraph = Paragraph::new(err_msg)
			.style(Style::default().fg(Color::LightRed))
			.alignment(Alignment::Center);
		frame.render_widget(error_paragraph, error_area);
	}

	let help_text = "Enter: Continue | Esc: Exit";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);
	frame.render_widget(help_widget, help_area);
}

fn render_onboarding_confirm_passphrase(
	frame: &mut Frame,
	passphrase_textarea: &mut TextArea,
	error: Option<&'static str>,
) {
	let main_block = Block::bordered()
		.title("Confirm Passphrase")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	frame.render_widget(main_block.clone(), frame.area());

	let inner_area = main_block.inner(frame.area());

	let [passphrase_area, error_area, help_area] =
		Layout::vertical([Constraint::Length(3), Constraint::Length(1), Constraint::Length(1)]).areas(inner_area);

	let passphrase_block = Block::bordered()
		.title("Passphrase")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));
	passphrase_textarea.set_block(passphrase_block);

	frame.render_widget(passphrase_textarea.deref(), passphrase_area);

	if let Some(err_msg) = error {
		let error_paragraph = Paragraph::new(err_msg)
			.style(Style::default().fg(Color::LightRed))
			.alignment(Alignment::Center);
		frame.render_widget(error_paragraph, error_area);
	}

	let help_text = "Enter: Confirm | Esc: Back";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);
	frame.render_widget(help_widget, help_area);
}

fn render_login(frame: &mut Frame, passphrase_textarea: &mut TextArea, error: Option<&'static str>) {
	let main_block = Block::bordered()
		.title("Login")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));

	frame.render_widget(main_block.clone(), frame.area());

	let inner_area = main_block.inner(frame.area());

	let [passphrase_area, error_area, help_area] =
		Layout::vertical([Constraint::Length(3), Constraint::Length(1), Constraint::Length(1)]).areas(inner_area);

	let passphrase_block = Block::bordered()
		.title("Passphrase")
		.title_alignment(Alignment::Center)
		.title_style(Style::default().fg(Color::LightCyan))
		.bg(Color::Black)
		.border_type(BorderType::Rounded)
		.border_style(Style::default().fg(Color::LightCyan));
	passphrase_textarea.set_block(passphrase_block);

	frame.render_widget(passphrase_textarea.deref(), passphrase_area);

	if let Some(err_msg) = error {
		let error_paragraph = Paragraph::new(err_msg)
			.style(Style::default().fg(Color::LightRed))
			.alignment(Alignment::Center);
		frame.render_widget(error_paragraph, error_area);
	}

	let help_text = "Enter: Login | Esc: Exit";
	let help_widget = Paragraph::new(help_text).fg(Color::Gray).alignment(Alignment::Center);
	frame.render_widget(help_widget, help_area);
}
