# Contributing to dx-ext

Thank you for considering contributing to dx-ext! This document outlines the process for contributing to the project and how to get started.

## Code of conduct

By participating in this project, you agree to abide by our Code of Conduct (to be created). Please report unacceptable behavior to the project maintainers.

## How Can I Contribute?

### Reporting Bugs

If you find a bug in the project, please create an issue using the following guidelines:

1. Check if the bug has already been reported
2. Use a clear and descriptive title
3. Describe the exact steps to reproduce the bug
4. Explain the behavior you expected to see
5. Include details about your environment (OS, Rust version, etc.)

### Suggesting Enhancements

We welcome suggestions for improvements:

1. Use a clear and descriptive title for your suggestion
2. Provide a detailed description of the suggested enhancement
3. Explain why this enhancement would be useful
4. List any alternatives you've considered

### Pull Requests

1. Fork the repository
2. Create a new branch for your changes
3. Make your changes
4. Run tests to ensure your changes don't break existing functionality
5. Submit a pull request

## Development setup

To set up your development environment:

1. Fork and clone the repository
2. Install Rust and Cargo if you haven't already
3. Install wasm-pack: `cargo install wasm-pack`
4. Run `cargo build` to ensure everything builds correctly

## Project Structure

The dx-ext tool is organized as follows:

- **Extension components**: Popup (UI), Background (persistent scripts), Content (web page scripts)
- **File operations**: Managed through the `EFile` enum
- **Build system**: Supports development and release modes
- **Watch system**: Monitors for file changes and rebuilds automatically

## Coding Guidelines

### Rust styles Guide

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` to format your code
- Use `clippy` to catch common mistakes

### Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests after the first line

## Testing

- Add tests for new features
- Ensure all tests pass before submitting your changes
- Consider adding integration tests for complex features

## Documentation

- Update the README.md with details of changes to the interface
- Update the example usage if applicable
- Comment your code where necessary, especially for complex logic

## Release Process

1. Version numbers follow [Semantic Versioning](https://semver.org/)
2. Changes for each release are documented in CHANGELOG.md
3. Releases are tagged in git and published to crates.io

## Questions?

If you have any questions about contributing, feel free to open an issue or contact the maintainers directly.

Thank you for contributing to dx-ext!
