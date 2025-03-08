# Dioxus Browser Extension Builder

A CLI tool for building and developing browser extension with Dioxus and WebAssembly.

## Overview

The Dioxus Browser Extension Builder (`dx-ext`) is a utility that simplifies the development and building of browser extensions using Dioxus (a Rust-based reactive UI framework)
and WebAssembly. This tool handles:

- Building WASM components from your Rust code
- Copying necessary assets and configuration files
- Providing a hot-reload development environment
- Managing the extension configuration via a TOML file

## Installation

### Prerequisites

- Rust(stable)
- wasm-paack

### From source

```bash
git clone https://github.com/Summit-Sailors/dioxus-browser-extension-builder.git
cd dioxus-browser-extension-builder
cargo install --path ./dx-ext

```

## Usage

### Basic Commands

```bash
dx-ext init # Generate a default comfig
dx-ext build # Build the extension (one time build)
dx-ext watch # Start the development server with hot reload

```

## Command Details

`dx-ext` init

Creates a default `dx-ext.toml` configuration file in the current directory.

```bash
dx-ext init

```

This will generate a configuration file in the current directory with default settings that you can customize for your extension

`dx-ext build`

Builds all crates and copies necessary files to the distribution directory without watching the changes.

```bash
dx-ext build

```

This command:

1. Builds all extension crates (popup, background, content) with `wasm-pack`
2. Copies all the required files to the distribution directory

`dx-ext watch`

Starts the file watcher and automatically rebuilds components when file changes

```bash
dx-ext watch

```

This Command:

1. Builds all extension components initially
2. Watches for file changes in the extension directory
3. Rebuilds and copies files as needed when changes are detected
4. Press ^C to stop the watcher

## Configuration

The tool is configured using a `dx-ext.toml` file in the porject root

```toml
[extension-config]
assets-directory = "popup/assets"                    # your assets directory relative to the extension directory
background-script-index-name = "background_index.js" # name of your background script entry point
content-script-index-name = "content_index.js"       # name of your content script entry point
extension-directory-name = "demo-extension"          # name of your extension directory
popup-name = "popup"                                 # name of your popup crate

```

### Configuration Options

| Option                         | Description                                                       | Default                 |
| ------------------------------ | ----------------------------------------------------------------- | ----------------------- |
| `assets-directory`             | Path to your assets directory relative to the extension directory | `"popup/assets"`        |
| `background-script-index-name` | Name of your background script entry point                        | `"background_index.js"` |
| `content-script-index-name`    | Name of your content script entry point                           | `"content_index.js"`    |
| `extension-directory-name`     | Name of your extension directory                                  | `"demo-extension"`      |
| `popup-name`                   | Name of your popup crate                                          | `"popup"`               |

## Project Structure

A typical project structure for a Dioxus browser extension:

```
your-extension/
├── dx-ext.toml                   # Extension configuration
├── your-extension-dir/           # Defined by extension-directory-name in config
│   ├── background/               # Background script crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── background_index.js       # Background script entry point
│   ├── content/                  # Content script crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── content_index.js          # Content script entry point
│   ├── dist/                     # Build output directory
│   ├── index.html                # Extension popup HTML
│   ├── index.js                  # Extension popup entry point
│   ├── manifest.json             # Chrome extension manifest
│   ├── popup/                    # Popup UI crate
│   │   ├── Cargo.toml
│   │   ├── assets/               # Static assets
│   │   └── src/
│   │       └── lib.rs
```

## Development Flow

1. Create a new extension project or navigate to an existing one
2. Configure your extension in `dx-ext.toml`
3. Run `dx-ext watch` to start the development server
4. Make changes to your Rust code or extension files
5. The tool will automatically rebuild and copy files as needed
6. Load your extension from the `dist` directory in your browser
7. When ready for production, run `dx-ext build` to create a final build

## Extension Components

The tool handles three main components of browser extensions:

### Popup

The popup UI that appears when clicking the extension icon. Built from the Rust crate specified by the `popup-name` configuration.

### Background Script

A script that runs in the background of the browser. Built from the `background` crate.

### Content Script

A script that runs in the context of web pages. Built from the `content` crate.

## Troubleshooting

### Common Issues

#### wasm-pack Build Failures

If you encounter build failures:

```
[FAIL] wasm-pack build for background failed with status: exit code: 1
```

Check the following:

- Ensure wasm-pack is correctly installed
- Verify your Rust code compiles without errors
- Check for incompatible dependencies

#### File Watcher Issues

If the file watcher isn't detecting changes:

- Ensure you're modifying files within the watched directories
- Try restarting the watcher
- Check file permissions

## Contributing

Contributions are welcome! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
