#[dotenvy::load(path = "../../.env")]
fn main() {
	if std::env::var("ENV").unwrap() == "local" {
		println!("cargo:rustc-env=RUST_BACKTRACE=1");
		println!("cargo:rustc-env=CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true");
		println!("cargo:rerun-if-changed=../.env");
	}

	for key in ["SERVER_URL", "ENV"] {
		println!("cargo:rustc-env={}={}", key, std::env::var(key).unwrap_or_else(|_| panic!("expected {key} env var")));
	}
}
