# Dioxus Browser Extension Builder

A complete browser extension environment using [Dioxus](https://dioxuslabs.com/) and Rust.

This workspace provides a foundation for building browser extensions with Rust and Dioxus. It includes all necessary components for a full-featured extension and uses the `dx-ext` CLI tool for development and building.

## To get Started

Install the `dx-ext` CLI tool:

```bash
cargo install dioxus-browser-extension-builder
```

Initialize the extension configuration:

```bash
dx-ext init
```

Start the development server:

```bash
dx-ext watch
```

Load the extension from the `extension/dist` directory in your browser

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
├── background/        # Background script crate
├── content/           # Content script crate
├── popup/             # Popup UI crate
├── common/            # Shared types and utilities
├── dist/              # Build output directory
├── manifest.json      # Extension manifest
├── background_index.js # Background script entry point
├── content_index.js   # Content script entry point
└── index.html         # Popup HTML template
```

## IMPORTANT

`crate-type` setting for [Dioxus](https://dioxuslabs.com/) browser extension crates: When building Dioxus browser extensions using `wasm-pack`, each crate you define in your project needs a specific setting in it's `Cargo.toml` file. This ensures proper compilation for both WebAssembly and Rust library usage.

Specifically, you'll need to add the following to your `[lib]` section:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

This is necessary because:

- `cdylib`(C Dynamic Library) tells rustc to compile your crate into a dynamic library that can be loaded and executed in a WebAssembly environment(like the browser). It's essential for creating the `.wasm` file that your extension will use.
- `rlib`(Rust Library) allows your crate to be used as a regular Rust library within your project. This is useful if you have other Rust code that depends on the crate.

By including `cdylib` and `rlib`, you ensure your crate is built in a way that satisfies both the WebAssembly requirements of your browser extension and rust library needs of your project.

## Contributing

Contributions are welcome! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Learn More

- [Dioxus Documentation](https://dioxuslabs.com/docs/)
- [Browser Extension CLI Tool](https://github.com/Summit-Sailors/dioxus-browser-extension-builder/blob/main/dx-ext/README.md)
