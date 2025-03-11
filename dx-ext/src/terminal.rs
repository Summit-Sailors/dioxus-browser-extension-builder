use {
	crate::{app::App, common::BuilState},
	crossterm::{
		ExecutableCommand,
		cursor::{Hide, Show},
		terminal::{disable_raw_mode, enable_raw_mode},
	},
	ratatui::{
		Frame,
		layout::{Constraint, Direction, Layout, Rect},
		style::{Color, Modifier, Style},
		symbols,
		widgets::{Block, BorderType, Borders, LineGauge, Paragraph},
	},
	std::{
		io::{self, stdout},
		rc::Rc,
	},
};

pub(crate) struct Terminal {
	pub(crate) terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
}

impl Terminal {
	pub(crate) fn new() -> io::Result<Self> {
		enable_raw_mode()?;
		let mut stdout = stdout();
		let _ = stdout.execute(Hide);

		let backend = ratatui::backend::CrosstermBackend::new(stdout);
		let terminal = ratatui::Terminal::new(backend)?;

		Ok(Self { terminal })
	}

	pub(crate) fn draw(&mut self, app: &mut App) -> io::Result<()> {
		self.terminal.draw(|frame| {
			let area = frame.area();

			// layout with a border
			let main_block = Block::default()
				.title("Dioxus Browser Extension Builder")
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
					Constraint::Length(1), // progress bar
					Constraint::Length(1), // status line
					Constraint::Length(3), // task list
					Constraint::Length(1), // empty space
					Constraint::Length(1), // instructions
				])
				.split(inner_area);

			// render task list
			Self::render_task_list(frame, chunks[0], app);

			// render status line
			Self::render_status(frame, chunks[1], app);

			// render the progress bar
			Self::render_progress_bar(frame, chunks[2], app);

			// render instructions
			frame.render_widget(Paragraph::new("Press 'r' to run/restart task, 'q' to quit").style(Style::default().fg(Color::Gray)), chunks[3]);
		})?;

		Ok(())
	}

	fn render_task_list(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
		let task_list = app.get_task_status();
		frame.render_widget(Paragraph::new(task_list).style(Style::default().fg(Color::White)), area);
	}

	fn render_progress_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
		let (progress, style, is_running) = match &app.task_state {
			BuilState::Idle => (0.0, Style::default().fg(Color::DarkGray), false),
			BuilState::Running { progress, .. } => (*progress, Style::default().fg(Color::Yellow), true),
			BuilState::Complete { .. } => (1.0, Style::default().fg(Color::Green), false),
			BuilState::Failed => (1.0, Style::default().fg(Color::Red), false),
		};

		let split_areas: Rc<[Rect]> = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Fill(1), Constraint::Length(12)]).split(area);

		let gauge_area = split_areas[0];
		let icon_area = split_areas[1];

		// render the progress gauge
		frame.render_widget(LineGauge::default().filled_style(style).line_set(symbols::line::THICK).ratio(progress).label("Progress: "), gauge_area);

		let split_icon_areas: Rc<[Rect]> =
			Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(3), Constraint::Fill(1)]).split(icon_area);

		let throbber_area = split_icon_areas[0];
		let time_area = split_icon_areas[1];

		if is_running {
			let throb = throbber_widgets_tui::Throbber::default()
				.style(Style::default().fg(Color::Cyan))
				.throbber_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
				.throbber_set(throbber_widgets_tui::BLACK_CIRCLE)
				.use_type(throbber_widgets_tui::WhichUse::Spin);

			frame.render_stateful_widget(throb, throbber_area, &mut app.throbber_state);
		} else {
			let status_icon = match app.task_state {
				BuilState::Complete { .. } => "✓ ",
				BuilState::Failed => "✗ ",
				_ => "  ",
			};

			frame.render_widget(
				Paragraph::new(status_icon).style(match app.task_state {
					BuilState::Complete { .. } => Style::default().fg(Color::Green),
					BuilState::Failed => Style::default().fg(Color::Red),
					_ => Style::default(),
				}),
				throbber_area,
			);
		}

		if let BuilState::Running { start_time, .. } = app.task_state {
			let elapsed = start_time.elapsed();
			frame.render_widget(Paragraph::new(format!("{:.1}s", elapsed.as_secs_f32())).style(Style::default().fg(Color::DarkGray)), time_area);
		} else if let BuilState::Complete { duration } = app.task_state {
			frame.render_widget(Paragraph::new(format!("{:.1}s", duration.as_secs_f32())).style(Style::default().fg(Color::DarkGray)), time_area);
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
			BuilState::Failed => "Task failed",
		};

		let status_style = match &app.task_state {
			BuilState::Idle => Style::default().fg(Color::Gray),
			BuilState::Running { .. } => Style::default().fg(Color::Yellow),
			BuilState::Complete { .. } => Style::default().fg(Color::Green),
			BuilState::Failed => Style::default().fg(Color::Red),
		};

		frame.render_widget(Paragraph::new(status_text).style(status_style), area);
	}
}

impl Drop for Terminal {
	fn drop(&mut self) {
		_ = disable_raw_mode();
		_ = self.terminal.backend_mut().execute(Show);
		_ = self.terminal.show_cursor();
	}
}
