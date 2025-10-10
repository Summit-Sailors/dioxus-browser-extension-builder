# Dioxus Browser Extension Builder

A complete browser extension environment using [Dioxus](https://dioxuslabs.com/) and Rust.

This workspace provides a foundation for building browser extensions with Rust and Dioxus. It includes all necessary components for a full-featured extension and uses the `dx-ext` CLI tool for development and building.

## To get Started

Install the `dx-ext` CLI tool:

```bash
cargo install dioxus-browser-extension-builder
```

Initialize the extension configuration and set up the workspace:

```bash
dx-ext init # For the default configuration
```

Or

```bash
dx-ext init -i # To be able to set up your preferred configuration
```

Start the development server:

```bash
dx-ext watch
```

Load the extension from the `extension/dist` directory to your browser

## Development Workflow

### Development Mode

```bash
dx-ext watch
```

This will:

- Build all components in development mode
- Watch for file changes
- Automatically rebuild when files change

### Production Build

```bash
dx-ext build --mode release
```

This will:

- Build all components in release mode
- Optimize for production
- Create a distributable extension in the `extension/dist` directory

## Typical Extension Project Structure

```tree
extension/
├── background/ # Background script crate
├── content/ # Content script crate
├── popup/ # Popup UI crate
├── common/ # Shared types and utilities
├── dist/ # Build output directory
├── manifest.json # Extension manifest
├── background_index.js # Background script entry point
├── content_index.js # Content script entry point
└── index.html # Popup HTML template
```

## IMPORTANT

### Crate Type Configuration

`crate-type` setting for [Dioxus](https://dioxuslabs.com/) browser extension crates: When building Dioxus browser extensions using `wasm-pack`, each crate you define in your project needs a specific setting in it's manifest file (`Cargo.toml`) file. This ensures proper compilation for both WebAssembly and Rust library usage.

Specifically, you'll need to add the following to your `[lib]` section:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

This is necessary because:

- `cdylib`(C Dynamic Library) tells rustc to compile your crate into a dynamic library that can be loaded and executed in a WebAssembly environment(like the browser). It's essential for creating the `.wasm` file that your extension will use.
- `rlib`(Rust Library) allows your crate to be used as a regular Rust library within your project. This is useful if you have other Rust code that depends on the crate.

By including `cdylib` and `rlib`, you ensure your crate is built in a way that satisfies both the WebAssembly requirements of your browser extension and rust library needs of your project.

### Known Issue: wasm-opt Bulk Memory Operations Error

**Issue Description:**
There's a compatibility issue between the Rust compiler's WebAssembly output and `wasm-opt` that affects both nightly and stable Rust versions. While the issue was first identified with Rust nightly `nightly-2025-02-18`, it has been confirmed to persist in stable Rust releases including **Rust 1.90.0**. The Rust compiler generates WebAssembly modules that use bulk memory operations (`memory.copy`, `memory.fill`), but `wasm-opt` fails to process these modules without explicit bulk memory support enabled.

**Error Symptoms:**
You may encounter errors like:

```
[wasm-validator error in function XXXX] unexpected false: Bulk memory operations require bulk memory [--enable-bulk-memory], on
(memory.copy
 ...
)
```

Or when trying to enable bulk memory support:

```
[wasm-validator error in function XXXX] unexpected false: all used features should be allowed, on
(i32.trunc_sat_f32_s
 ...
)
```

**Background:**
This issue affects projects using:

- **Both stable and nightly Rust versions** (confirmed in Rust 1.90.0 and nightly builds from `nightly-2025-02-18` onwards)
- `wasm-pack` with `wasm-opt` optimization enabled
- Dioxus, wasm-bindgen, and other WebAssembly frameworks

**Current Solutions:**

**Option 1: Disable wasm-opt (Recommended for now)**

Add the following to your crate's `Cargo.toml`:

```toml
[package.metadata.wasm-pack.profile.profiling]
wasm-opt = false

[package.metadata.wasm-pack.profile.release]
wasm-opt = false
```

This disables WebAssembly optimization but ensures your build completes successfully.

**Option 2: Attempt bulk-memory flag (May not work)**

Some users have reported limited success with:

```toml
[package.metadata.wasm-pack.profile.profiling]
wasm-opt = ['-O', '--enable-bulk-memory']

[package.metadata.wasm-pack.profile.release]
wasm-opt = ['-O', '--enable-bulk-memory']
```

**Note:** This approach often fails with additional validation errors and may require disabling automatic TOML formatting to prevent array reordering.

**Alternative Workarounds:**

1. **Use an older Rust version:** If possible, downgrade to a Rust version before this issue was introduced (though this may not be practical for projects requiring newer language features)
2. **Manual wasm-opt:** Build without wasm-opt, then manually run `wasm-opt` with appropriate flags:

```bash
wasm-opt -Oz --enable-bulk-memory input.wasm -o output.wasm
```

**Status and Future:**
This is an active issue being tracked in the Rust repository ([rust-lang/rust#137315](https://github.com/rust-lang/rust/issues/137315)). The Rust team is working on a fix, but until resolved, using **Option 1** (disabling wasm-opt) is the most reliable approach.

**Impact on Development:**

- Development builds are unaffected
- Production builds will be larger without wasm-opt optimization
- Functionality remains intact, only file size optimization is lost

## Contributing

Contributions are welcome! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Learn More

- [Browser Extension CLI Tool](https://github.com/Summit-Sailors/dioxus-browser-extension-builder/blob/main/dx-ext/README.md)
- [Dioxus Documentation](https://dioxuslabs.com/docs/)
- [Related Issue: Rust nightly wasm-opt compatibility](https://github.com/rust-lang/rust/issues/137315)
