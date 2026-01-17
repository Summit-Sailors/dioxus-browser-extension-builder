set shell := ["bash", "-uc"]

default:
  @just --choose --justfile {{justfile()}}

# Clean all build artifacts (target, lock files, pkg dirs, dist)
clean:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "Cleaning build artifacts..."

  # Remove Cargo.lock
  if [ -f Cargo.lock ]; then
    rm -f Cargo.lock
    echo "  ✓ Removed Cargo.lock"
  fi

  # Remove target directory
  if [ -d target ]; then
    rm -rf target
    echo "  ✓ Removed target/"
  fi

  # Remove WASM pkg directories
  for dir in demo-extension/*/pkg; do
    if [ -d "$dir" ]; then
      rm -rf "$dir"
      echo "  ✓ Removed $dir"
    fi
  done

  # Remove dist directory
  if [ -d demo-extension/dist ]; then
    rm -rf demo-extension/dist
    echo "  ✓ Removed demo-extension/dist/"
  fi

  echo ""
  echo "Project is now clean. You can reinstall dx-ext with:"
  echo "  cargo install --path dx-ext"

# Clean including cargo git cache (for dependency issues)
clean-all:
  #!/usr/bin/env bash
  set -euo pipefail
  just clean

  echo ""
  echo "Cleaning cargo git cache..."

  # Clear cargo git cache for dioxus-free-icons (common source of issues)
  if [ -d ~/.cargo/git/checkouts/dioxus-free-icons-* ]; then
    rm -rf ~/.cargo/git/checkouts/dioxus-free-icons-*
    echo "  ✓ Cleared dioxus-free-icons git cache"
  fi

  # Clear cargo git cache for dotenvy
  if [ -d ~/.cargo/git/checkouts/dotenvy-* ]; then
    rm -rf ~/.cargo/git/checkouts/dotenvy-*
    echo "  ✓ Cleared dotenvy git cache"
  fi

  # Clear cargo git cache for schemars
  if [ -d ~/.cargo/git/checkouts/schemars-* ]; then
    rm -rf ~/.cargo/git/checkouts/schemars-*
    echo "  ✓ Cleared schemars git cache"
  fi

  echo ""
  echo "Full clean complete."

# Sort derive attributes
sort-d:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo sort-derives

# Run extension watcher for development
ext-watch:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo run -p dioxus-browser-extension-builder watch

# Run the demo server
demo-server:
  #!/usr/bin/env bash
  set -euo pipefail
  dx serve --server -p server

# Check all workspace members compile
check:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo check --workspace

# Run all tests
test:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo test --workspace

# Run clippy lints
clippy:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo clippy --workspace -- -D warnings

# Format code
fmt:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo fmt --all

# Check formatting
fmt-check:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo fmt --all -- --check

# Build the dx-ext CLI tool
build-cli:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo build -p dioxus-browser-extension-builder

# Build all workspace members
build:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo build --workspace

# Build in release mode
build-release:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo build --workspace --release

# Build demo extension WASM components
build-wasm:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "Building popup..."
  cd demo-extension/popup && wasm-pack build --target web --dev
  echo "Building background..."
  cd demo-extension/background && wasm-pack build --target no-modules --dev
  echo "Building content..."
  cd demo-extension/content && wasm-pack build --target web --dev
  echo "Building options..."
  cd demo-extension/options && wasm-pack build --target web --dev
  echo "All WASM builds complete!"

# Build demo extension WASM in release mode
build-wasm-release:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "Building popup (release)..."
  cd demo-extension/popup && wasm-pack build --target web --release
  echo "Building background (release)..."
  cd demo-extension/background && wasm-pack build --target no-modules --release
  echo "Building content (release)..."
  cd demo-extension/content && wasm-pack build --target web --release
  echo "Building options (release)..."
  cd demo-extension/options && wasm-pack build --target web --release
  echo "All WASM release builds complete!"

# Run all verification checks (check, test, clippy, fmt-check)
verify:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "=== Running cargo check ==="
  cargo check --workspace
  echo ""
  echo "=== Running tests ==="
  cargo test --workspace
  echo ""
  echo "=== Running clippy ==="
  cargo clippy --workspace -- -D warnings
  echo ""
  echo "=== Checking formatting ==="
  cargo fmt --all -- --check
  echo ""
  echo "All verification checks passed!"

# Full CI pipeline (verify + build-wasm)
ci:
  #!/usr/bin/env bash
  set -euo pipefail
  just verify
  echo ""
  echo "=== Building WASM components ==="
  just build-wasm
  echo ""
  echo "CI pipeline complete!"

# Show dx-ext CLI help
help-cli:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo run -p dioxus-browser-extension-builder -- --help