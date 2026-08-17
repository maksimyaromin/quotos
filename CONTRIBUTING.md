# Contributing

Thanks for looking at Quotos. This page covers how to get set up, what
has to pass before a change lands, and what to expect after opening a
pull request.

## Setup

1. Clone the repository and run `npm install` before opening it in an
   editor. Every TypeScript project here resolves its own types from
   `node_modules`, so skipping this step shows up as editor errors that
   clear once you run it.
2. Rust needs Xcode's command line tools to link on macOS:
   `xcode-select --install`.
3. `npm run tauri dev` runs the real app. `npm run dev` runs the
   interface against mock data in a browser, for anything that does not
   need the native shell. [README.md](README.md) has the full set of
   run, build, and test commands.

## The gate

`npm run verify` has to be green before a pull request can land. It
runs formatting and linting, both TypeScript projects, the test suite,
a handful of repository checks, and the Rust equivalents: `cargo fmt`
and `cargo clippy`. CI runs the same command on every pull request and
on every push to `main`.

[docs/contributing.md](docs/contributing.md) covers what the gate
enforces and why: naming, module layout, the design system's styling
rules, and where new code belongs. Read it before a change that adds a
file or touches more than one part of the tree.

## Commits

Commit messages follow Conventional Commits: `type(scope): summary`,
for example `fix(rust): correct the docked panel offset`.

## Opening a pull request

Keep a pull request to one change, and fill in the template: what
changed, why, and how you tested it. CI has to pass before it can
merge.

For anything bigger than a small fix, especially a change to how a
surface behaves, open an issue first. It can save you from writing a
change that does not fit where the project is headed.
