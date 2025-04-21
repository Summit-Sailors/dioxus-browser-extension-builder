use {
	crate::{
		EXMessage,
		app::App,
		common::{BuilState, BuildStatus},
	},
	ratatui::{
		Frame,
		crossterm::{
			self, cursor,
			event::{self, KeyCode, KeyEventKind},
			terminal::{EnterAlternateScreen, LeaveAlternateScreen},
		},
		layout::{Constraint, Direction, Layout, Rect},
		prelude::CrosstermBackend,
		style::{Color, Modifier, Style},
		symbols,
		text::{Line, Span},
		widgets::{Block, BorderType, Borders, LineGauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
	},
	std::{
		io::{self, stderr},
		ops::{Deref, DerefMut},
		sync::Arc,
		time::Duration,
	},
	tokio::sync::{Mutex, mpsc},
	tokio_util::sync::CancellationToken,
	tracing::error,
};

const TICK_RATE_MS: u64 = 100;

pub(crate) struct Terminal {
	pub terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
	pub cancellation_token: CancellationToken,
	pub app: Arc<Mutex<App>>,
	pub ui_rx: mpsc::UnboundedReceiver<EXMessage>,
	pub ui_tx: mpsc::UnboundedSender<EXMessage>,
}

impl Terminal {
	pub fn new() -> Result<Self, io::Error> {
		let backend = ratatui::backend::CrosstermBackend::new(stderr());
		let terminal = ratatui::Terminal::new(backend)?;
		let cancellation_token = CancellationToken::new();
		let app = Arc::new(Mutex::new(App::new()));
		let (ui_tx, ui_rx) = mpsc::unbounded_channel();

		Ok(Self { terminal, cancellation_token, app, ui_rx, ui_tx })
	}

	pub(crate) fn exit_tui() -> Result<(), io::Error> {
		crossterm::terminal::disable_raw_mode()?;
		crossterm::execute!(std::io::stderr(), LeaveAlternateScreen, cursor::Show)?;
		Ok(())
	}

	pub async fn start(&mut self) -> Result<(), io::Error> {
		crossterm::terminal::enable_raw_mode()?;
		crossterm::execute!(std::io::stderr(), EnterAlternateScreen, cursor::Hide)?;
		let mut interval = tokio::time::interval(Duration::from_millis(TICK_RATE_MS));
		let key_event_filter = |key: &KeyCode| -> bool { matches!(key, KeyCode::Char('q' | 'r') | KeyCode::Up | KeyCode::Down) };

		loop {
			tokio::select! {
				_ = self.cancellation_token.cancelled() => {
					Self::exit_tui()?;
					break;
				}
				_ = interval.tick() => {
					if event::poll(Duration::from_millis(0))? {
						let mut app = self.app.lock().await;
						match event::read()? {
							event::Event::Key(key) => {
								if key.kind == KeyEventKind::Press && key_event_filter(&key.code) {
									app.update(EXMessage::Keypress(key.code)).await;
								}
							}
							event::Event::Mouse(mouse_event) => {
								app.update(EXMessage::Mouse(mouse_event)).await;
							}
							event::Event::Paste(content) => {
								app.update(EXMessage::Paste(content)).await;
							}
							_ => {}
						}
					}

					if !self.process_update(EXMessage::Tick).await? {
						return Ok(());
					}
				}
				Some(ui_msg) = self.ui_rx.recv() => {
					if !self.process_update(ui_msg).await? {
						return Ok(());
					}
				}
			}
		}
		Ok(())
	}

	async fn process_update(&mut self, message: EXMessage) -> Result<bool, io::Error> {
		let should_quit;
		{
			let mut app = self.app.lock().await;
			app.update(message).await;
			should_quit = app.should_quit;
		}
		if should_quit {
			self.cancellation_token.cancel();
			Self::exit_tui()?;
			return Ok(false);
		}
		if let Err(e) = self.draw().await {
			error!("Failed to draw UI: {}", e);
			return Err(e);
		}
		Ok(true) // continue the loop
	}

	pub async fn draw(&mut self) -> io::Result<()> {
		let mut app = self.app.lock().await;
		self.terminal.draw(|frame| {
			let area = frame.area();

			// layout with a border
			let main_block = Block::default()
				.title(Line::from(Span::styled("Dioxus Browser Extension Builder", Style::default().fg(Color::White))).centered())
				.borders(Borders::ALL)
				.border_type(BorderType::Rounded)
				.border_style(Style::default().fg(ratatui::style::Color::DarkGray));

			let inner_area = main_block.inner(area);
			frame.render_widget(main_block, area);

			// split inner area into sections
			let chunks = Layout::default()
				.direction(ratatui::layout::Direction::Vertical)
				.margin(1)
				.constraints([
					Constraint::Length(3),   // task status area
					Constraint::Length(1),   // progress bar
					Constraint::Length(1),   // status line
					Constraint::Length(100), // logs area (fills remaining space)
					Constraint::Length(1),   // instructions
				])
				.split(inner_area);

			// render task list
			Self::render_task_list(frame, chunks[0], &app);
			// render status line
			Self::render_status(frame, chunks[2], &app);
			// render the progress bar
			Self::render_progress_bar(frame, chunks[1], &mut app);
			// render logs
			Self::render_logs(frame, chunks[3], &mut app);

			// render instructions
			frame.render_widget(
				Paragraph::new("Press 'r' to run/restart task, 'q' to quit, Use Up and Down keys to scroll through the logs")
					.centered()
					.style(Style::default().fg(Color::Gray)),
				chunks[4],
			);
		})?;

		Ok(())
	}

	fn render_block(title: &str) -> Block<'_> {
		Block::default()
			.title(Line::from(Span::styled(title, Style::default().fg(Color::Cyan))).centered())
			.borders(Borders::ALL)
			.border_type(BorderType::Rounded)
			.border_style(Style::default().fg(Color::DarkGray))
	}

	fn render_logs(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
		let logs_block = Self::render_block("Logs");
		frame.render_widget(&logs_block, area);
		let inner_area = logs_block.inner(area);

		let max_visible_logs = inner_area.height as usize;
		app.max_logs = max_visible_logs;
		let total_logs = app.log_buffer.len();
		let max_scroll = total_logs.saturating_sub(max_visible_logs);

		// ensure scroll offset stays within bounds
		if app.scroll_offset > max_scroll || !app.user_scrolled {
			app.scroll_offset = max_scroll;
		}

		let log_items: Vec<ListItem<'_>> = app.log_buffer.iter().skip(app.scroll_offset).take(max_visible_logs).cloned().map(ListItem::new).collect();
		let logs_list = List::new(log_items).block(Block::default()).style(Style::default());

		frame.render_widget(logs_list, inner_area);

		let content_length = total_logs.max(max_visible_logs);
		let mut scrollbar_state = ScrollbarState::default().position(app.scroll_offset).content_length(content_length);

		frame.render_stateful_widget(
			Scrollbar::new(ScrollbarOrientation::VerticalRight).begin_symbol(Some("↑")).end_symbol(Some("↓")),
			inner_area,
			&mut scrollbar_state,
		);
	}

	fn render_task_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
		let tasks_block = Self::render_block("Tasks");
		let inner_area = tasks_block.inner(area);

		frame.render_widget(tasks_block, area);
		let tasks_text = app.get_task_status();
		let tasks_paragraph = Paragraph::new(tasks_text).centered().style(Style::default().fg(Color::White));

		frame.render_widget(tasks_paragraph, inner_area);
	}

	fn render_progress_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
		let (progress, style, label, is_running) = if !app.has_active_tasks() {
			(0.0, Style::default().fg(Color::DarkGray), " No active tasks ".to_owned(), false)
		} else {
			let (total, pending, in_progress, _completed) = app.get_task_stats();
			let failed = app.tasks.values().filter(|&&s| s == BuildStatus::Failed).count();
			let success = app.tasks.values().filter(|&&s| s == BuildStatus::Success).count();

			match &app.task_state {
				BuilState::Idle => {
					if pending > 0 {
						(0.0, Style::default().fg(Color::Yellow), format!(" Preparing {} task{} ", total, if total != 1 { "s" } else { "" }), false)
					} else {
						(0.0, Style::default().fg(Color::DarkGray), format!(" Waiting to start {} task{} ", total, if total != 1 { "s" } else { "" }), false)
					}
				},

				BuilState::Running { progress, .. } => {
					let style = if *progress < 0.66 { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Green) };
					let percent = (progress * 100.0).round();
					let label = format!(" {percent:.0}% | {success}/{total} completed, {in_progress}/{total} in progress, {pending} pending, {failed} failed ");

					(*progress, style, label, true)
				},

				BuilState::Complete { duration } => {
					let time_str = if duration.as_secs() >= 60 {
						format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
					} else {
						format!("{:.1}s", duration.as_secs_f32())
					};

					(1.0, Style::default().fg(Color::Green), format!(" Complete ({success}/{total} tasks) in {time_str} "), false)
				},

				BuilState::Failed { duration } => {
					let time_str = if duration.as_secs() >= 60 {
						format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
					} else {
						format!("{:.1}s", duration.as_secs_f32())
					};

					(1.0, Style::default().fg(Color::Red), format!(" Failed ({failed}/{total} tasks failed) in {time_str} "), false)
				},
			}
		};

		// A centered progress bar for status indicators
		let split_areas = Layout::default()
			.direction(Direction::Horizontal)
			.constraints([
				Constraint::Percentage(10), // left margin
				Constraint::Percentage(80), // progress bar
				Constraint::Percentage(10), // status indicators
			])
			.split(area);
		let gauge_area = split_areas[1];
		let icon_area = split_areas[2];

		// the progress gauge with label
		frame.render_widget(LineGauge::default().filled_style(style).line_set(symbols::line::THICK).ratio(progress).label(label), gauge_area);

		let split_icon_areas = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(3), Constraint::Fill(1)]).split(icon_area);
		let throbber_area = split_icon_areas[0];
		let time_area = split_icon_areas[1];

		if is_running {
			let throb = throbber_widgets_tui::Throbber::default()
				.style(Style::default().fg(Color::Cyan))
				.throbber_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
				.throbber_set(throbber_widgets_tui::BLACK_CIRCLE)
				.use_type(throbber_widgets_tui::WhichUse::Spin);

			frame.render_stateful_widget(throb, throbber_area, &mut app.throbber_state);

			// elapsed time for running tasks
			if let Some(start_time) = app.overall_start_time {
				let elapsed = start_time.elapsed();
				let time_text =
					if elapsed.as_secs() >= 60 { format!("{}m {}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60) } else { format!("{:.1}s", elapsed.as_secs_f32()) };

				frame.render_widget(Paragraph::new(time_text).style(Style::default().fg(Color::DarkGray)), time_area);
			}
		} else {
			let status_icon = match app.task_state {
				BuilState::Complete { .. } => "✓ ",
				BuilState::Failed { .. } => "✗ ",
				_ => " ",
			};

			let icon_style = match app.task_state {
				BuilState::Complete { .. } => Style::default().fg(Color::Green),
				BuilState::Failed { .. } => Style::default().fg(Color::Red),
				_ => Style::default(),
			};

			frame.render_widget(Paragraph::new(status_icon).style(icon_style), throbber_area);

			// completion time for finished tasks
			if let BuilState::Complete { duration } = app.task_state {
				let time_text = if duration.as_secs() >= 60 {
					format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
				} else {
					format!("{:.1}s", duration.as_secs_f32())
				};
				frame.render_widget(Paragraph::new(time_text).style(Style::default().fg(Color::DarkGray)), time_area);
			}
		}
	}

	fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App) {
		let status_text = match &app.task_state {
			BuilState::Idle => "Ready to run task",
			BuilState::Running { progress, .. } => {
				if *progress < 0.33 {
					"Starting task..."
				} else if *progress < 0.66 {
					"Task in progress"
				} else {
					"Task almost complete"
				}
			},
			BuilState::Complete { .. } => "Task completed successfully",
			BuilState::Failed { .. } => "Task failed",
		};

		let status_style = match &app.task_state {
			BuilState::Idle => Style::default().fg(Color::Gray),
			BuilState::Running { .. } => Style::default().fg(Color::Yellow),
			BuilState::Complete { .. } => Style::default().fg(Color::Green),
			BuilState::Failed { .. } => Style::default().fg(Color::Red),
		};

		frame.render_widget(Paragraph::new(status_text).alignment(ratatui::layout::Alignment::Center).style(status_style), area);
	}
}

impl Deref for Terminal {
	type Target = ratatui::Terminal<CrosstermBackend<std::io::Stderr>>;

	fn deref(&self) -> &Self::Target {
		&self.terminal
	}
}

impl DerefMut for Terminal {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.terminal
	}
}

impl Drop for Terminal {
	fn drop(&mut self) {
		let _ = Self::exit_tui();
	}
}
