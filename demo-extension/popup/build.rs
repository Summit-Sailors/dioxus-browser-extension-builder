use std::process::Command;

#[dotenvy::load(path = "../../.env")]
fn main() {
	if std::env::var("ENV").unwrap() == "local" {
		println!("cargo:rustc-env=RUST_BACKTRACE=1");
		println!("cargo:rustc-env=CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true");
		println!("cargo:rerun-if-changed=../.env");
		println!("cargo:rerun-if-changed=./input.css");
		println!("cargo:rerun-if-changed=./tailwind.config.js");
	}
	for key in ["SERVER_URL", "ENV"] {
		println!("cargo:rustc-env={}={}", key, std::env::var(key).unwrap_or_else(|_| panic!("expected {key} env var")));
	}
	let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
	let mut cmd = Command::new("npx");
	let mut args = vec!["tailwindcss", "-i", "./input.css", "-o", "./assets/tailwind.css"];
	if profile == "release" {
		args.push("--minify");
	}
	let output = cmd.args(&args).output().expect("Failed to execute tailwindcss command");
	if !output.status.success() {
		panic!("Tailwind CSS compilation failed: {}", String::from_utf8_lossy(&output.stderr));
	}
}
