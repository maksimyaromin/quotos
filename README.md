# Quotos

A macOS menu bar utility that shows, at a glance, how much of each Claude
subscription is left.

Quotos tracks more than one account at once, reading each directly from
the machine with no server and no account of its own. It currently
supports Claude; the design leaves room for other providers without
assuming a single one anywhere. See [the design brief](docs/design/brief.md)
for the full product specification and [the architecture](docs/architecture.md)
for how the application is built.

## Requirements

- macOS
- Node.js 24 or newer, the version `.nvmrc` pins
- a Rust toolchain, since Tauri builds a native backend alongside the web
  frontend

## Install

```bash
npm install
```

Run this before opening the repository in an editor, not only before
running a command. Every TypeScript project in the tree resolves its own
dependency types from `node_modules`, and an uninstalled tree shows
editor errors that an install clears.

## Run

```bash
npm run tauri dev
```

This runs the real application: a status item in the menu bar that opens
a panel on click.

To exercise every interface state without the native shell, against mock
data, in any browser:

```bash
npm run dev
```

Then open http://localhost:1420/.

## Test

```bash
npm test                       # vitest
npm run typecheck              # TypeScript
(cd src-tauri && cargo test)   # Rust
```

## Build

```bash
npm run tauri build
```

The bundle lands at `src-tauri/target/release/bundle/macos/Quotos.app`.

## Verify

```bash
npm run verify
```

The single quality gate: formatting and linting, both TypeScript
projects, the test suite, a handful of repository checks, and the Rust
equivalents. [AGENTS.md](AGENTS.md) covers what each lane checks and
where the rest of the documentation lives.

## License

[MIT](LICENSE)
