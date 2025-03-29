use {
	crate::{
		BuildMode, EFile, ExtensionCrate, LogLevel, PENDING_BUILDS, PENDING_COPIES,
		common::{BuilState, BuildStatus, EXMessage, TaskState},
		read_config,
	},
	crossterm::event::KeyCode,
	ratatui::{
		style::{Color, Style},
		text::{Line, Span},
	},
	std::{
		collections::HashMap,
		time::{Duration, Instant},
	},
	strum::IntoEnumIterator,
};

#[derive(Debug, Clone)]
pub(crate) struct App {
	pub task_state: BuilState,
	pub should_quit: bool,
	pub throbber_state: throbber_widgets_tui::ThrobberState,
	pub tasks: HashMap<String, BuildStatus>,
	pub task_history: HashMap<String, TaskState>,
	pub log_buffer: Vec<Line<'static>>,
	pub scroll_offset: usize,
	pub max_logs: usize,
	pub overall_start_time: Option<Instant>,
}

impl App {
	pub fn new() -> Self {
		Self {
			task_state: BuilState::Idle,
			should_quit: false,
			throbber_state: throbber_widgets_tui::ThrobberState::default(),
			tasks: HashMap::new(),
			task_history: HashMap::new(),
			log_buffer: Vec::new(),
			scroll_offset: 0,
			max_logs: 100,
			overall_start_time: None,
		}
	}

	pub fn has_active_tasks(&self) -> bool {
		!self.tasks.is_empty()
	}

	// overall progress with a weighted system
	pub fn calculate_overall_progress(&self) -> f64 {
		if self.tasks.is_empty() {
			return 0.0;
		}

		let total_tasks = self.tasks.len() as f64;
		let mut total_progress = 0.0;

		for (task_name, status) in &self.tasks {
			let task_weight = 1.0 / total_tasks;
			let task_progress = match status {
				BuildStatus::Success => 1.0,
				BuildStatus::Failed => 1.0,
				BuildStatus::InProgress => {
					// trying to get more granular progress for in-progress tasks
					if let Some(task_state) = self.task_history.get(task_name) {
						task_state.progress.unwrap_or(0.5)
					} else {
						0.5 // default to 50% if no detailed progress is available
					}
				},
				BuildStatus::Pending => 0.0,
			};

			total_progress += task_weight * task_progress;
		}

		total_progress.max(0.0).min(1.0)
	}

	pub fn get_task_stats(&self) -> (usize, usize, usize, usize) {
		let total = self.tasks.len();
		let pending = self.tasks.values().filter(|&&s| s == BuildStatus::Pending).count();
		let in_progress = self.tasks.values().filter(|&&s| s == BuildStatus::InProgress).count();
		let completed = self.tasks.values().filter(|&&s| s == BuildStatus::Success).count();
		let failed = self.tasks.values().filter(|&&s| s == BuildStatus::Failed).count();

		(total, pending, in_progress, completed + failed)
	}

	// update task state and recalculate progress
	pub fn update_task(&mut self, task_name: String, status: BuildStatus) {
		if !self.task_history.contains_key(&task_name) {
			self.task_history.insert(task_name.clone(), TaskState::default());
		}

		let task_state = self.task_history.get_mut(&task_name).unwrap();
		let now = Instant::now();

		match (task_state.status, status) {
			(BuildStatus::Pending, BuildStatus::InProgress) => {
				task_state.start_time = Some(now);
				task_state.progress = Some(0.0);

				if self.overall_start_time.is_none() {
					self.overall_start_time = Some(now);
				}
			},

			(BuildStatus::InProgress, BuildStatus::Success | BuildStatus::Failed) => {
				task_state.end_time = Some(now);
				task_state.progress = Some(1.0);
			},

			_ => {},
		}

		task_state.status = status;
		self.tasks.insert(task_name, status);

		// recalculate overall build state
		self.update_overall_state();
	}

	fn update_overall_state(&mut self) {
		if self.tasks.is_empty() {
			self.task_state = BuilState::Idle;
			return;
		}

		let (total, pending, in_progress, finished) = self.get_task_stats();

		if pending + in_progress == 0 && finished == total {
			if self.tasks.values().any(|&status| status == BuildStatus::Failed) {
				if let Some(start_time) = self.overall_start_time {
					let duration = start_time.elapsed();
					self.task_state = BuilState::Failed { duration };
				} else {
					self.task_state = BuilState::Failed { duration: Duration::from_secs(0) };
				}
			} else if let Some(start_time) = self.overall_start_time {
				let duration = start_time.elapsed();
				self.task_state = BuilState::Complete { duration };
			} else {
				self.task_state = BuilState::Complete { duration: Duration::from_secs(0) };
			}
		} else if in_progress > 0 || (pending > 0 && finished > 0) {
			let progress = self.calculate_overall_progress();
			if let BuilState::Running { start_time, .. } = self.task_state {
				self.task_state = BuilState::Running { progress, start_time };
			} else {
				let start_time = self.overall_start_time.unwrap_or_else(Instant::now);
				self.task_state = BuilState::Running { progress, start_time };
			}
		} else if pending == total && in_progress == 0 {
			self.task_state = BuilState::Idle;
		}
	}

	pub fn update_task_progress(&mut self, task_name: &str, progress: f64) {
		if let Some(task_state) = self.task_history.get_mut(task_name) {
			task_state.progress = Some(progress.max(0.0).min(1.0));
		}

		// recalculate overall progress
		if let BuilState::Running { start_time, .. } = self.task_state {
			let overall_progress = self.calculate_overall_progress();
			self.task_state = BuilState::Running { progress: overall_progress, start_time };
		}
	}

	pub fn get_task_status(&self) -> String {
		if self.tasks.is_empty() {
			return "No active tasks".to_string();
		}

		let mut result = String::new();
		let task_count = self.tasks.len();
		let mut completed = 0;

		for (task, status) in &self.tasks {
			let status_symbol = match status {
				BuildStatus::Pending => "⏳",
				BuildStatus::InProgress => "🔁",
				BuildStatus::Success => {
					completed += 1;
					"✅"
				},
				BuildStatus::Failed => {
					completed += 1;
					"❌"
				},
			};

			result.push_str(&format!("{} {} ", status_symbol, task));

			// separators between tasks
			if completed < task_count {
				result.push_str(" | ");
			}
		}

		result
	}
	pub async fn update(&mut self, message: EXMessage) {
		match message {
			EXMessage::Keypress(key) => match key {
				KeyCode::Char('q') => {
					self.should_quit = true;
				},
				KeyCode::Char('r') => {
					self.reset().await;
				},
				KeyCode::Up => {
					if self.scroll_offset > 0 {
						self.scroll_offset = self.scroll_offset.saturating_sub(1);
					}
				},
				KeyCode::Down => {
					if self.scroll_offset < self.log_buffer.len().saturating_sub(1) {
						self.scroll_offset += 1;
					}
				},
				_ => {},
			},
			EXMessage::Tick => {
				self.throbber_state.calc_next();
			},
			EXMessage::BuildProgress(progress) => {
				if let BuilState::Running { start_time, .. } = self.task_state {
					self.task_state = BuilState::Running { progress, start_time }
				}
			},
			EXMessage::BuildComplete => {
				if let BuilState::Running { start_time, .. } = self.task_state {
					let duration = start_time.elapsed();
					self.task_state = BuilState::Complete { duration }
				}
			},
			EXMessage::BuildFailed => {
				if let BuilState::Running { start_time, .. } = self.task_state {
					let duration = start_time.elapsed();
					self.task_state = BuilState::Failed { duration };
				} else {
					self.task_state = BuilState::Failed { duration: Duration::from_secs(0) };
				}
			},
			EXMessage::UpdateTask(task_name, status) => {
				self.update_task(task_name, status);
			},
			EXMessage::TaskProgress(task_name, progress) => {
				self.update_task_progress(&task_name, progress);
			},
			EXMessage::LogMessage(level, msg) => {
				self.add_log(level, &msg);
			},
		}
	}

	pub fn add_log(&mut self, level: LogLevel, message: &str) {
		let (prefix, color) = match level {
			LogLevel::Debug => ("[DEBUG]", Color::Blue),
			LogLevel::Info => ("[INFO] ", Color::Green),
			LogLevel::Warn => ("[WARN] ", Color::Yellow),
			LogLevel::Error => ("[ERROR]", Color::Red),
		};

		let config = read_config().expect("Failed to read config");

		if matches!(config.build_mode, BuildMode::Release) && matches!(prefix, "[DEBUG]") {
			return;
		}

		let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

		let log_line = Line::from(vec![
			Span::styled(format!("{} ", timestamp), Style::default().fg(Color::DarkGray)),
			Span::styled(prefix, Style::default().fg(color)),
			Span::styled(format!(" {}", message), Style::default()),
		]);

		self.log_buffer.push(log_line);

		if self.log_buffer.len() > self.max_logs {
			let excess = self.log_buffer.len() - self.max_logs;
			self.log_buffer.drain(0..excess);
		}
	}

	pub async fn reset(&mut self) {
		self.log_buffer.clear();
		self.add_log(LogLevel::Info, "Resetting application state...");

		self.tasks.clear();
		self.task_history.clear();
		self.overall_start_time = Some(Instant::now());
		self.task_state = BuilState::Running { progress: 0.0, start_time: Instant::now() };
		self.throbber_state.normalize(&throbber_widgets_tui::Throbber::default());

		self.add_log(LogLevel::Info, "Initializing tasks...");
		for e_crate in ExtensionCrate::iter() {
			PENDING_BUILDS.lock().await.insert(e_crate);
			self.tasks.insert(e_crate.get_task_name(), BuildStatus::Pending);
			self.task_history.insert(e_crate.get_task_name(), TaskState::default());
		}

		for e_file in EFile::iter() {
			PENDING_COPIES.lock().await.insert(e_file);
		}

		self.add_log(LogLevel::Info, "Reset complete, awaiting rebuild...");
	}
}
