use std::process::Command;

fn main() {
	println!("cargo:rustc-env=RUST_BACKTRACE=1");
	println!("cargo:rustc-env=CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true");

	let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8071".to_string());
	let env = std::env::var("ENV").unwrap_or_else(|_| "Local".to_string());

	println!("cargo:rustc-env=SERVER_URL={}", server_url);
	println!("cargo:rustc-env=ENV={}", env);

	println!("cargo:rerun-if-changed=./input.css");
	println!("cargo:rerun-if-changed=./tailwind.config.js");

	let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
	let mut cmd = Command::new("npx");
	let mut args = vec!["tailwindcss", "-i", "./input.css", "-o", "./assets/tailwind.css"];

	if profile == "release" {
		args.push("--minify");
	}

	let output = cmd.args(&args).output().expect("Failed to execute Tailwind CSS command");
	if !output.status.success() {
		panic!("Tailwind CSS compilation failed: {}", String::from_utf8_lossy(&output.stderr));
	}
}
