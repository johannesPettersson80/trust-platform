# Contributing to truST Platform

Thanks for taking the time to contribute. This project is early-stage and still moving fast.

## Development setup

Prerequisites:
- Rust 1.85+
- Node.js 20+ (for the VS Code extension)

Clone and build:

```bash
git clone https://github.com/johannesPettersson80/trust-platform
cd trust-platform
cargo build
```

Run formatting and checks:

```bash
just fmt
just clippy
just test
```

## VS Code extension (optional)

```bash
cd editors/vscode
npm install
npm run compile
```

## Pull requests

- Keep PRs focused and small when possible.
- Update docs/specs when behavior changes.
- Add or update tests for new behavior.
- Use clear commit messages (imperative mood).

## Code style

- Rust: `cargo fmt` is required.
- Clippy: `cargo clippy --all-targets --all-features -- -D warnings` should pass.

## Reporting issues

Use GitHub Issues with a minimal repro and expected vs. actual behavior.
