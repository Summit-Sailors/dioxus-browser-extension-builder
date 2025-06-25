# Dioxus Browser Extension Builder

A CLI tool for building Dioxus browser extensions.

## Overview

The Dioxus Browser Extension Builder (`dx-ext`) is a utility that simplifies the development and building of browser extensions using [Dioxus](https://dioxuslabs.com/)
This tool handles:

- Setting up a project workspace with a basic browser extension template
- Building WASM components from your Rust code
- Copying necessary assets and configuration files
- Providing a hot-reload development environment
- Managing the extension configuration via a TOML file

## Installation

### Prerequisites

- Rust (stable)
- wasm-pack

### From source

```bash
git clone https://github.com/Summit-Sailors/dioxus-browser-extension-builder.git
cd dioxus-browser-extension-builder
cargo install --path ./dx-ext
```

### From [crates.io](https://crates.io/crates/dioxus-browser-extension-builder)

```bash
cargo install dioxus-browser-extension-builder
```

## Quick Start

```bash
# Generate a default configuration file
dx-ext init

# Build the extension (one-time build)
dx-ext build

# Start the development server with hot reload
dx-ext watch
```

Note: Use the `--help` or `-h` flag on each command for more information

## Command Details

### `dx-ext init`

Sets up a workspace and creates a default `dx-ext.toml` configuration file in the current directory.

```bash
# Create with default values
dx-ext init

# Create with custom values
dx-ext init --extension-dir my-extension --popup-name my-popup --background-script bg.js --content-script cs.js --assets-dir assets

# Create interactively
dx-ext init --interactive / dx-ext init -i

# Overwrite the existing config file
dx-ext init --force / dx-ext init -f
```

Options:

- `--extension-dir`: Name of your extension directory (default: "extension")
- `--popup-name`: Name of your popup crate (default: "popup")
- `--background-script`: Name of your background script entry point (default: "background_index.js")
- `--content-script`: Name of your content script entry point (default: "content_index.js")
- `--assets-dir`: Your assets directory relative to the extension directory (default: "popup/assets")
- `--force, -f`: Force overwrite of existing config file
- `--interactive, -i`: Interactive mode to collect configuration
- `--mode, -m`: Build mode: development or release (default: "development")
- `--clean, -c`: Clean build (remove dist directory first)

### `dx-ext build`

Builds all crates in the specified mode (`dev` or `prod` -> default-> `dev`) and copies necessary files to the distribution directory without watching for changes.

```bash
dx-ext build

# For clean builds
dx-ext build --clean # to remove previous build artifacts

# For release builds
dx-ext build --mode release
```

This command:

1. Builds all extension crates (popup, background, content) with `wasm-pack`
2. Copies all the required files to the distribution directory

### `dx-ext watch`

Starts the file watcher and automatically rebuilds components when files change.

```bash
dx-ext watch

# For clean re-builds
dx-ext watch --clean # to remove previous build artifacts

# For release re-builds
dx-ext watxh --mode release
```

This command:

1. Builds all extension components initially
2. Watches for file changes in the extension directory
3. Rebuilds and copies files as needed when changes are detected
4. Press `q` to stop the watcher
5. Prss `r` to restart the watcher

## Configuration

The tool is configured using a `dx-ext.toml` file in the project root(Workspace):

```toml
[extension-config]
assets-directory = "popup/assets"                    # your assets directory relative to the extension directory
background-script-index-name = "background_index.js" # name of your background script entry point
content-script-index-name = "content_index.js"       # name of your content script entry point
enable-incremental-builds = false                    # enable incremental builds for watch command
extension-directory-name = "extension"               # name of your extension directory
popup-name = "popup"                                 # name of your popup crate
```

### Configuration Options

| Option                         | Description                                                       | Default                 |
| ------------------------------ | ----------------------------------------------------------------- | ----------------------- |
| `assets-directory`             | Path to your assets directory relative to the extension directory | `"popup/assets"`        |
| `background-script-index-name` | Name of your background script entry point                        | `"background_index.js"` |
| `content-script-index-name`    | Name of your content script entry point                           | `"content_index.js"`    |
| `extension-directory-name`     | Name of your extension directory                                  | `"extension"`           |
| `enable-incremental-builds`    | Enable incremental builds for watch command                       | `false`                 |
| `popup-name`                   | Name of your popup crate                                          | `"popup"`               |

## Project Structure

A typical project structure for a Dioxus browser extension:

```tree
your-project/
├── dx-ext.toml # Extension configuration
├── extension/ # Defined by extension-directory-name in config
│ ├── background/ # Background script crate
│ │ ├── Cargo.toml
│ │ └── src/
│ │ └── lib.rs
│ ├── background_index.js # Background script entry point
│ ├── content/ # Content script crate
│ │ ├── Cargo.toml
│ │ └── src/
│ │ └── lib.rs
│ ├── content_index.js # Content script entry point
│ ├── dist/ # Build output directory
│ ├── index.html # Extension popup HTML
│ ├── index.js # Extension popup entry point
│ ├── manifest.json # Chrome extension manifest
│ ├── popup/ # Popup UI crate
│ │ ├── Cargo.toml
│ │ ├── assets/ # Static assets
│ │ └── src/
│ │ └── lib.rs

```

## Development Flow

1. Run `dx-ext init` to create the default configuration, or customize with options (use `-i` flag)
2. Adjust your extension settings in `dx-ext.toml` if needed
3. Run `dx-ext watch` to start the development server
4. Make changes to your Rust code or extension files
5. The tool will automatically rebuild and copy files as needed
6. Load your extension from the `dist` directory in your browser
7. When ready for production, run `dx-ext build -m prod` or `dx-ext build -m release` to create a release build

## File Watching

The watcher monitors:

- All source files in the crate directories for changes
- Extension configuration files (manifest.json, HTML, JS files)
- Shared API code (when detected in the paths)
- Assets directory

Changes trigger specific rebuilds:

- Changes to source files rebuild only the affected crates
- Changes to shared API code rebuild all crates
- Changes to extension files only copy the modified files

## Performance Features

- Parallel file copying operations using Rayon
- Debounced builds to prevent multiple rebuilds when many files change at once
- Cancellation token system for graceful shutdown
- Asynchronous operations for non-blocking performance

## License

This project is licensed under the MIT License - see the LICENSE file for details.
