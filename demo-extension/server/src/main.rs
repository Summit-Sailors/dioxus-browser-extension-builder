use dioxus::server::axum::Router;

#[allow(unused_imports)]
use server::*;

fn main() {
	dioxus::logger::initialize_default();
	dioxus::serve(|| async {
		// Create a plain router - server functions are registered automatically
		Ok::<Router, anyhow::Error>(Router::new())
	});
}
