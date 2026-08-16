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

## Design system styling

Every component styles itself with a sibling CSS Module,
`ComponentName.module.css`, that reads the token layer under
`src/design-system/tokens/` through `var()`. `src/design-system/tokens/`
stays the one source of truth for a value; a component's stylesheet only
ever references a token, never restates one.

A prop that selects among a closed, small set of variants, a tone, a
size, a boolean flag such as `pinned` or `expanded`, becomes a
`data-*` attribute on the styled element, matched in the module CSS with
an attribute selector: `[data-variant="primary"]`. A state the platform
already tracks, hover, active, focus, disabled, is a real pseudo-class in
the module CSS, never a `useState` plus mouse handlers standing in for
one. A value that is genuinely per-render, a measured height, a computed
menu position, a percentage fill width, stays an inline `style`, since
neither a CSS Module class nor a token layer can express something only
known at render time. `joinClassNames` (`src/design-system/joinClassNames.ts`)
composes a component's own module class with a caller-supplied
`className`, so a consumer can layer its own layout constraints in CSS
rather than only through the `style` prop. See [Testing](#testing) below
for how a spec verifies a CSS Module's rules rather than an inline style.

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
  `@/*` path alias, target ES2024.
- `tsconfig.node.json` covers `vite.config.ts`: Node types, bundler
  module resolution to match how Vite itself loads the file, target
  ES2023.
- `tools/tsconfig.json` covers the repository scripts under `tools/`,
  described below.

`src`'s target follows the macOS floor the application supports:
`src-tauri/tauri.conf.json`'s `minimumSystemVersion` is macOS 15, whose
initial release shipped Safari 18.0. ES2024 is fully supported by that
version. Its own two most recent additions, `Object.groupBy` /
`Map.groupBy` and `Promise.withResolvers`, shipped in Safari 17.4;
the rest of the year's spec — `Array.fromAsync`, resizable and
transferable `ArrayBuffer`, growable `SharedArrayBuffer`, and the
well-formed string methods — had already shipped in Safari 16.4, and
the RegExp `v` flag in Safari 17.0. Safari 18 carries the complete set
well before its own release. Vite does not polyfill missing runtime
APIs for an older engine, only lowers syntax, so a feature outside
`lib` reaches that floor as a runtime crash, not a caught typecheck
error.

`tools/` and `vite.config.ts` stay on ES2023, tracking the Node runtime
that actually executes them — `package.json`'s `engines.node` is
`>=24` — not the macOS floor. The app project's target used to land on
the same spec year by coincidence, because Safari 17 fully supported
it; the two have now diverged exactly as expected, since they track
genuinely different runtimes and this macOS floor bump landed on a
newer Safari's ceiling than ES2023.

Vite's own production `build.target` is a separate setting from any of
the above, pinned in `vite.config.ts` to `"safari18"` to match
`minimumSystemVersion` exactly, rather than left at Vite's default,
`baseline-widely-available`. That default tracks whatever the Baseline
initiative currently calls widely available, which moves forward on its
own schedule and would drift out of step with the declared floor again
without a person deciding to move it.

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
import's real content instead of its usual empty-string stub for CSS; the
same option also covers plain `.module.css` imports, so a spec that
renders a component gets that component's real CSS Module rules applied
in jsdom, and `getComputedStyle` reflects them rather than the browser's
initial values. jsdom cannot parse a multi-value `transition` shorthand
back into its longhand computed properties, so a spec pinning one still
reads the stylesheet's own text through `?raw`, the same as a token file.

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
