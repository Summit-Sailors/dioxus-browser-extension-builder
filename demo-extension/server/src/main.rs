use dioxus::{prelude::*, server::axum::Router};

#[allow(unused_imports)]
use server::*;

fn main() {
	dioxus::logger::initialize_default();
	dioxus::serve(|| async { Ok(Router::new().register_server_functions()) });
}
