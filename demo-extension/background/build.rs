fn main() {
	println!("cargo:rustc-env=RUST_BACKTRACE=1");
	println!("cargo:rustc-env=CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true");

	let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8071".to_string());
	let env = std::env::var("ENV").unwrap_or_else(|_| "Local".to_string());

	println!("cargo:rustc-env=SERVER_URL={}", server_url);
	println!("cargo:rustc-env=ENV={}", env);
}
