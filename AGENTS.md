# Project agent memory

This file is what an agent or a new contributor reads before touching
this repository: only what almost every visit here needs, with pointers
to the rest.

## What this is

Tauri v2 + React + TypeScript menu bar app for macOS, rooted at the
repository root, so `npm`, `vitest`, `tsc`, and `tauri` all run from
here. Application code and repository scripts are both TypeScript; see
[docs/contributing.md](docs/contributing.md) for the `tools/` setup and
the module-layout rule. `npm run verify` is the single quality gate,
running format, lint, typecheck, test, `cargo fmt`, `cargo clippy`, and a
few small repository checks concurrently, reporting every failing lane.
CI runs the identical command on every pull request and on every push
to the default branch. `README.md` covers day-to-day run, build, and
test commands.

## Sources of truth

- `docs/design/brief.md` is the product spec: the entity model, every
  state, every surface.
- `docs/design/prototype.html` is the interactive reference. Where the
  running app and the prototype disagree, the prototype wins.
- `src/design-system/` holds the design system: the tokens and React
  components the app builds against, and the only copy of them in this
  repository.
- `docs/brand/` holds the brand assets; `docs/contributing.md`'s "App
  icons" section names every source of truth the mark and the wordmark
  are drawn from, in the app and out of it.
- [docs/architecture.md](docs/architecture.md),
  [docs/claude-provider.md](docs/claude-provider.md), and
  [docs/platform-constraints.md](docs/platform-constraints.md) cover how
  the pieces fit together and the non-obvious constraints they run into.
- [docs/contributing.md](docs/contributing.md) covers the conventions and
  the gate.
<!-- anb:begin -->- `.agent-notebook/` is the project notebook: the
  decisions and standing rules behind the code, as plain Markdown.
  `anb recall` reads it; so does opening the files.<!-- anb:end -->
- [.agents/anb.md](.agents/anb.md) is how that notebook is kept here:
  what earns a record, how it is named, and what must never go in one.

## Architecture

The Tauri process boundary splits ownership cleanly: Rust owns the
operating system, native windows, the Keychain, and every HTTP request;
React owns what gets drawn. The two sides talk only through a small set
of commands and events. Adding a usage provider beyond Claude means a
new module on each side of `src/providers/registry.ts` and
`src-tauri/src/providers/mod.rs`, plus one registry entry, and nothing
else references a provider by name.
[docs/architecture.md](docs/architecture.md) has the full map.

## Maintaining this file

Keep this file short: only knowledge almost every reader needs, with a
pointer to the rest. Durable technical detail belongs in `docs/`, in the
page nearest the behavior it describes, written in the present tense,
not appended here. Point to the authoritative file or command instead of
repeating what the code already shows. Prune guidance that has stopped
being true instead of annotating it.
