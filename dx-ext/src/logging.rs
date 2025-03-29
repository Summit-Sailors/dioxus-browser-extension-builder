use {
	std::sync::Arc,
	tokio::sync::Mutex,
	tracing::{Event, Subscriber, field::Visit},
	tracing_subscriber::{Layer, registry::LookupSpan},
};

// type alias for a logging callback function
pub(crate) type LogCallback = Arc<Mutex<dyn Fn(LogLevel, &str) + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
	Debug,
	Info,
	Warn,
	Error,
}

// custom layer for tracing (that will forward logs to TUI)
pub(crate) struct TUILogLayer {
	callback: LogCallback,
}

impl TUILogLayer {
	pub fn new(callback: LogCallback) -> Self {
		Self { callback }
	}
}

impl<S> Layer<S> for TUILogLayer
where
	S: Subscriber + for<'a> LookupSpan<'a>,
{
	fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
		// log message extraction
		let mut message = String::new();

		event.record(&mut MessageVisitor(&mut message));

		let level = match *event.metadata().level() {
			tracing::Level::DEBUG => LogLevel::Debug,
			tracing::Level::INFO => LogLevel::Info,
			tracing::Level::WARN => LogLevel::Warn,
			_ => LogLevel::Error,
		};

		// Send the log to the TUI via callback
		let callback = self.callback.clone();
		tokio::spawn(async move {
			let callback_guard = callback.lock().await;
			(callback_guard)(level, &message);
		});
	}
}

struct MessageVisitor<'a>(&'a mut String);

impl Visit for MessageVisitor<'_> {
	fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			self.0.push_str(&format!("{value:?}"));
		}
	}

	fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
		if field.name() == "message" {
			self.0.push_str(value);
		}
	}
}
