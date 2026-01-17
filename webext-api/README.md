# webext-api

Type-safe Rust bindings for browser extension APIs, compiled to WebAssembly.

## Overview

This crate provides idiomatic Rust APIs for interacting with browser extension APIs (Chrome, Firefox). It's used by `dx-ext` built extensions to access browser functionality from WASM.

## Supported APIs

- `action` - Browser action (toolbar button)
- `alarms` - Scheduling periodic tasks
- `commands` - Keyboard shortcuts
- `context_menus` - Right-click context menus
- `runtime` - Extension lifecycle and messaging
- `scripting` - Content script injection
- `storage` - Local/sync storage
- `tabs` - Tab management
- `side_panel` - Side panel UI
- `declarative_net_request` - Network request modification (Chrome only)

## Usage

```rust
use webext_api::{init, BrowserType};

let browser = init()?;
let tabs = browser.tabs().query_current_tab().await?;
```

## Features

- `chrome` - Chrome-specific APIs
- `firefox` - Firefox-specific APIs
