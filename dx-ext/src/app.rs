use {
	crate::common::{BuilState, BuildStatus, EXMessage},
	crossterm::event::KeyCode,
	std::{collections::HashMap, time::Instant},
};
pub(crate) struct App {
	pub(crate) task_state: BuilState,
	pub(crate) should_quit: bool,
	pub(crate) throbber_state: throbber_widgets_tui::ThrobberState,
	pub(crate) tasks: HashMap<String, BuildStatus>,
}

impl App {
	pub(crate) fn new() -> Self {
		Self { task_state: BuilState::Idle, should_quit: false, throbber_state: throbber_widgets_tui::ThrobberState::default(), tasks: HashMap::new() }
	}
	pub(crate) fn get_task_status(&self) -> String {
		if self.tasks.is_empty() {
			return "No active tasks".to_string();
		}
		let mut lines = Vec::new();
		for (task, status) in &self.tasks {
			let status_indicator = match status {
				BuildStatus::Pending => "⏳",
				BuildStatus::InProgress => "🔁",
				BuildStatus::Success => "✅",
				BuildStatus::Failed => "❌",
			};
			lines.push(format!("{} {}", status_indicator, task));
		}
		lines.join("\n")
	}
	pub(crate) fn update(&mut self, message: EXMessage) {
		match message {
			EXMessage::Keypress(key) => match key {
				KeyCode::Char('q') => {
					self.should_quit = true;
				},
				KeyCode::Char('r') => {
					self.task_state = BuilState::Running { progress: 0.0, start_time: Instant::now() };
				},
				_ => {},
			},
			EXMessage::Tick => {
				// update throbber animation on each tick
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
				self.task_state = BuilState::Failed;
			},
			EXMessage::Exit => {
				self.should_quit = true;
			},
			EXMessage::UpdateTask(task_name, status) => {
				self.tasks.insert(task_name, status);
				// update overall progress based on tasks
				let total_tasks = self.tasks.len();
				if total_tasks > 0 {
					let completed_tasks = self.tasks.values().filter(|&status| matches!(status, BuildStatus::Success | BuildStatus::Failed)).count();
					if completed_tasks == total_tasks {
						if self.tasks.values().any(|&status| matches!(status, BuildStatus::Failed)) {
							self.update(EXMessage::BuildFailed);
						} else {
							self.update(EXMessage::BuildComplete);
						}
					} else if completed_tasks > 0 {
						if let BuilState::Running { start_time, .. } = self.task_state {
							let progress = completed_tasks as f64 / total_tasks as f64;
							self.task_state = BuilState::Running { progress, start_time };
						}
					}
				}
			},
		}
	}
}
