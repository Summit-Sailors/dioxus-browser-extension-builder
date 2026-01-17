# server

Demo backend server for the browser extension example.

## Overview

This crate provides a Dioxus fullstack server that handles API requests from the browser extension. It re-exports server functions from `common` and runs the `/api/summarize` endpoint.

## Running

```bash
dx serve --server -p server
```

Or via justfile:

```bash
just demo-server
```

## Relationship to Other Crates

- Uses `common` with the `server` feature enabled for server function implementations
- The `background` script in the extension calls this server's API endpoints via HTTP
