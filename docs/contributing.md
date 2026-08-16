# Contributing

Conventions for working in this repository, and what the gate checks
before a change lands. `README.md` covers day-to-day run, build, and test
commands; this page covers how the code is shaped and proven.

## The gate

`npm run verify` is the single quality gate: format and lint through
Biome, a TypeScript typecheck of each of the tree's TypeScript projects
separately, the vitest suite, a handful of small repository checks
including this documentation's link check, `cargo fmt --check`,
`cargo clippy -- -D warnings`, and `cargo test`. `tools/verify.ts` is the
source of truth for the exact lane list; every lane runs concurrently,
and every failure is reported, not just the first. CI runs the identical
command on every pull request.

## Module layout

There is exactly one path alias, `@/*` resolving to `./src/*`, defined
once in `tsconfig.app.json`'s own `paths`. Every internal package is reached
through that one prefix: `@/lib/...`, `@/hooks/...`, `@/providers/...`,
and the design system itself as `@/design-system`, which resolves to
`src/design-system/index.ts` through the same wildcard's ordinary
directory-index resolution rather than a second, design-system-specific
entry. Vite and vitest read the same `paths` field natively, so there is
one alias definition, not one per tool.

The rule for when to alias: an import crossing from one top-level `src/`
directory into another, or from a root file such as `App.tsx` into any
of them, uses `@/...`. An import staying inside one top-level directory's
own tree, including the design system talking to itself, stays relative.
`grep -rn 'from "\.\./' src` should only ever match design system
internals and one same-module relative import in
`providers/claude/index.ts`.

Code outside `src/design-system/` reaches it only through
`src/design-system/index.ts`, a hand-curated barrel of named exports,
never through a component's own file path. A component's API name stays
stable unless it is genuinely unprofessional, since an external design
tool regenerates that surface; flag a naming concern there instead of
renaming around it.

## TypeScript configuration

The tree is a solution-style set of project references. The root
`tsconfig.json` holds only `references`, so an editor opening any file
finds the right project without being told which one to use.
`tsconfig.base.json` holds the options every project shares — `strict`,
`skipLibCheck`, `isolatedModules`, `noEmit`, `noUnusedLocals`,
`noUnusedParameters`, `noFallthroughCasesInSwitch` — and each leaf
project extends it, then sets only what makes that project different.
Strictness, module resolution and library level are each decided once,
in the project that owns the decision, not restated per file.

Three leaf projects exist, and every TypeScript file in the tree belongs
to exactly one of them; `tsc --showConfig -p <project>` answers, for any
file, which project claims it and with which options:

- `tsconfig.app.json` covers `src`: DOM libraries, no Node types, the
  `@/*` path alias, target ES2021.
- `tsconfig.node.json` covers `vite.config.ts`: Node types, bundler
  module resolution to match how Vite itself loads the file, target
  ES2023.
- `tools/tsconfig.json` covers the repository scripts under `tools/`,
  described below.

`src`'s target follows the macOS floor the application supports:
`src-tauri/tauri.conf.json`'s `minimumSystemVersion` is macOS 11, whose
initial release shipped Safari 14.0.1. ES2021 is the highest level that
version can run — caniuse.com lists Safari 14 as the first version
supporting both ES2021 syntax such as the `||=`/`&&=`/`??=` logical
assignment operators and ES2021 library additions such as `Promise.any`,
while ES2022 additions such as `Array.prototype.at` need Safari 15.4.
Vite does not polyfill missing runtime APIs for an older engine, only
lowers syntax, so a feature outside `lib` reaches that floor as a runtime
crash, not a caught typecheck error.

`tools/` and `vite.config.ts` target ES2023 instead, matching the Node
runtime that actually executes them — `package.json`'s `engines.node` is
`>=24` — because Node's own capability is not bounded by Safari's, and
holding the tooling back to the browser's ceiling would forbid real,
currently-available Node APIs for no runtime reason. The two targets
track two genuinely different runtimes rather than one drifting away
from the other by accident.

Vite's own production `build.target` is a separate setting from any of
the above, currently left at Vite's default, `baseline-widely-available`,
which resolves to Chrome 111, Edge 111, Firefox 114, Safari 16.4 and iOS
16.4. That default is newer than the ES2021-era floor
`src-tauri/tauri.conf.json` states, and Vite does the tree's only real
emission, so a tsconfig `target` change cannot fix that gap on its own.
Bringing them back into agreement is a product decision, either bumping
`minimumSystemVersion` to match what the bundle already assumes or
pinning `vite.config.ts`'s `build.target` down to the stated floor, and
belongs to whoever owns that call.

## `tools/`

`tools/` is TypeScript that Node runs directly, with no build step and no
loader flag, relying on Node's built-in type stripping. Its
`erasableSyntaxOnly` setting means a script that drifts into a construct
Node's stripper cannot erase fails `npm run typecheck:tools` instead of
only at runtime, and its `nodenext` module resolution matches Node's own
ESM resolution rules exactly, unlike the bundler resolution the app and
`vite.config.ts` projects use.

## Naming

Names use the domain's own vocabulary, sieved to its plainest term.
macOS calls the thing in the menu bar a status item; `Tray` is Tauri's
own word and stays only where the code touches Tauri's API directly,
such as `tauri::tray::TrayIcon`. A rename is swept across the whole tree
by search, then searched again for the old shape.

## Comments

The default is deletion. A comment earns its line by explaining an idea
the code cannot make obvious on its own, marking a `TODO`, or documenting
a genuine public surface: the commands the frontend invokes across the
process boundary, or the design system's exported components. A comment
that only restates its own line, however formally written, is deleted.

## Testing

Each source file has one matching spec file, enforced by the `.spec`
suffix check; a stray `.test.` file fails the gate. Vitest runs against
jsdom, which has no real layout engine, so anything that depends on
actual on-screen geometry needs verifying by hand against `npm run tauri
dev` or a real bundle, not asserted in a spec.

A spec that needs a stylesheet's actual text, rather than jsdom's
rendered styles, reads it through Vite's own `?raw` import suffix rather
than Node's `fs`, for example `import elevation from
"./tokens/elevation.css?raw"` in `reducedMotion.spec.tsx`. That keeps the
spec inside the same DOM-only project as every other file under `src`.
`vite.config.ts`'s `test.css.include` is what makes Vitest serve that
import's real content instead of its usual empty-string stub for CSS.

## App icons

`src/design-system/assets/app-icon.svg` is the source vector behind
`src-tauri/icons/icon.icns`; `menubar-glyph.svg` in the same directory
is the source behind the procedurally rendered status item glyph in
`status_item_render.rs`, so a glyph change only needs that file's
constants updated, never a new raster export. Regenerating the app icon
itself has no automated pipeline: rasterize the SVG at each of the
standard icon sizes, 16 through 1024, pack the results into a standard
`.iconset` directory, and convert with `iconutil -c icns`, which ships
with Xcode's command line tools.

## Commits

Commits follow Conventional Commits: `type(scope): summary`, for example
`fix(rust): correct the docked panel offset`.
