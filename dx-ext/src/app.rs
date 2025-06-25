use {
	crate::{
		BuildMode, EFile, ExtensionCrate, LogLevel, PENDING_BUILDS, PENDING_COPIES,
		common::{BuildState, EXMessage, TaskState, TaskStats, TaskStatus},
		read_config,
	},
	ratatui::{
		crossterm::event::KeyCode,
		style::{Color, Style},
		text::{Line, Span},
	},
	std::{collections::HashMap, time::Instant},
	strum::IntoEnumIterator,
};

static LOG_BUFFER_SIZE: usize = 1000;

#[derive(Debug, Clone)]
pub(crate) struct App {
	pub task_state: BuildState,
	pub should_quit: bool,
	pub throbber_state: throbber_widgets_tui::ThrobberState,
	pub tasks: HashMap<String, TaskStatus>,
	pub task_history: HashMap<String, TaskState>,
	pub log_buffer: Vec<Line<'static>>,
	pub scroll_offset: usize,
	pub user_scrolled: bool,
	pub max_logs: usize,
	pub overall_start_time: Option<Instant>,
}

impl App {
	pub fn new() -> Self {
		Self {
			task_state: BuildState::Idle,
			should_quit: false,
			throbber_state: throbber_widgets_tui::ThrobberState::default(),
			tasks: HashMap::new(),
			task_history: HashMap::new(),
			log_buffer: Vec::new(),
			scroll_offset: 0,
			user_scrolled: false,
			max_logs: 0,
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

		let total_weight: f64 = self.task_history.values().map(|task| task.weight).sum();
		if total_weight == 0.0 {
			return 0.0;
		}

		let mut weighted_progress = 0.0;

		for (task_name, status) in &self.tasks {
			let task_state = self.task_history.get(task_name);
			let weight = task_state.map_or(1.0, |ts| ts.weight);

			let task_progress = match status {
				TaskStatus::Failed | TaskStatus::Success => 1.0,
				TaskStatus::InProgress => {
					task_state.and_then(|ts| ts.progress).unwrap_or(0.1) // Small progress for started tasks
				},
				TaskStatus::Pending => 0.0,
			};

			weighted_progress += (weight / total_weight) * task_progress;
		}

		weighted_progress.clamp(0.0, 1.0)
	}

	pub fn get_task_stats(&self) -> TaskStats {
		let total = self.tasks.len();
		let pending = self.tasks.values().filter(|&&s| s == TaskStatus::Pending).count();
		let in_progress = self.tasks.values().filter(|&&s| s == TaskStatus::InProgress).count();
		let completed = self.tasks.values().filter(|&&s| s == TaskStatus::Success).count();
		let failed = self.tasks.values().filter(|&&s| s == TaskStatus::Failed).count();

		TaskStats { total, pending, in_progress, completed, failed }
	}

	// update task state and recalculate progress
	pub fn update_task(&mut self, task_name: String, status: TaskStatus) {
		if !self.task_history.contains_key(&task_name) {
			self.task_history.insert(task_name.clone(), TaskState::default());
		}

		let task_state = self.task_history.get_mut(&task_name).expect("Task state should exist after insertion");
		let now = Instant::now();

		// state transitions handling
		match (task_state.status, status) {
			(TaskStatus::Pending, TaskStatus::InProgress) => {
				task_state.start_time = Some(now);
				task_state.progress = Some(0.0);

				// set overall start time if this is the first task
				if self.overall_start_time.is_none() {
					self.overall_start_time = Some(now);
				}
			},
			(TaskStatus::InProgress, TaskStatus::Success | TaskStatus::Failed) => {
				task_state.end_time = Some(now);
				task_state.progress = Some(1.0);
			},
			_ => {},
		}

		task_state.status = status;
		self.tasks.insert(task_name, status);

		self.update_overall_state();
	}

	fn update_overall_state(&mut self) {
		if self.tasks.is_empty() {
			self.task_state = BuildState::Idle;
			return;
		}

		let stats = self.get_task_stats();

		// overall state based on task statistics
		match (stats.pending, stats.in_progress, stats.failed, stats.completed) {
			// all tasks completed successfully
			(0, 0, 0, completed) if completed == stats.total => {
				let duration = self.overall_start_time.map(|start| start.elapsed()).unwrap_or_default();
				self.task_state = BuildState::Complete { duration };
			},

			// some tasks failed
			(_, _, failed, _) if failed > 0 && stats.pending + stats.in_progress == 0 => {
				let duration = self.overall_start_time.map(|start| start.elapsed()).unwrap_or_default();
				self.task_state = BuildState::Failed { duration };
			},

			// tasks are running
			(_, in_progress, _, _) if in_progress > 0 => {
				let progress = self.calculate_overall_progress();
				let start_time = match self.task_state {
					BuildState::Running { start_time, .. } => start_time,
					_ => self.overall_start_time.unwrap_or_else(Instant::now),
				};
				self.task_state = BuildState::Running { progress, start_time };
			},

			// all pending
			(pending, 0, 0, 0) if pending == stats.total => {
				self.task_state = BuildState::Idle;
			},

			// mixed state - some completed, some pending
			_ => {
				let progress = self.calculate_overall_progress();
				let start_time = self.overall_start_time.unwrap_or_else(Instant::now);
				self.task_state = BuildState::Running { progress, start_time };
			},
		}
	}

	pub fn update_task_progress(&mut self, task_name: &str, progress: f64) {
		if let Some(task_state) = self.task_history.get_mut(task_name) {
			task_state.progress = Some(progress.clamp(0.0, 1.0));
		}

		// recalculate overall progress
		if let BuildState::Running { start_time, .. } = self.task_state {
			let overall_progress = self.calculate_overall_progress();
			self.task_state = BuildState::Running { progress: overall_progress, start_time };
		}
	}

	pub fn get_task_status(&self) -> String {
		if self.tasks.is_empty() {
			return "No active tasks".to_owned();
		}

		let mut result = String::new();
		let task_count = self.tasks.len();
		let mut completed = 0;

		for (task, status) in &self.tasks {
			let status_symbol = match status {
				TaskStatus::Pending => "⏳",
				TaskStatus::InProgress => "🔁",
				TaskStatus::Success => {
					completed += 1;
					"✅"
				},
				TaskStatus::Failed => {
					completed += 1;
					"❌"
				},
			};

			result.push_str(&format!("{status_symbol} {task} "));

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
						self.scroll_offset = self.scroll_offset.saturating_sub(5);
						if !self.user_scrolled {
							self.user_scrolled = true;
						}
					}
				},
				KeyCode::Down => {
					if self.scroll_offset < self.log_buffer.len().saturating_sub(5) && self.user_scrolled {
						self.scroll_offset += 5;
						self.user_scrolled = true;
					}
				},
				_ => {},
			},
			EXMessage::Mouse(_mouse_event) => {},
			EXMessage::Paste(_content) => {},
			EXMessage::Tick => {
				self.throbber_state.calc_next();
			},
			EXMessage::BuildProgress(progress) => {
				if let BuildState::Running { start_time, .. } = self.task_state {
					self.task_state = BuildState::Running { progress, start_time }
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
			Span::styled(format!("{timestamp} "), Style::default().fg(Color::DarkGray)),
			Span::styled(prefix, Style::default().fg(color)),
			Span::styled(format!(" {message}"), Style::default()),
		]);
		self.log_buffer.push(log_line);

		if self.log_buffer.len() > LOG_BUFFER_SIZE {
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
		self.task_state = BuildState::Running { progress: 0.0, start_time: Instant::now() };
		self.throbber_state.normalize(&throbber_widgets_tui::Throbber::default());
		self.user_scrolled = false;

		self.add_log(LogLevel::Info, "Initializing tasks...");
		for e_crate in ExtensionCrate::iter() {
			PENDING_BUILDS.lock().await.insert(e_crate);
			self.tasks.insert(e_crate.get_task_name(), TaskStatus::Pending);
			self.task_history.insert(e_crate.get_task_name(), TaskState::default());
		}

		for e_file in EFile::iter() {
			PENDING_COPIES.lock().await.insert(e_file);
		}
		self.add_log(LogLevel::Info, "Reset complete, awaiting rebuild...");
	}
}
