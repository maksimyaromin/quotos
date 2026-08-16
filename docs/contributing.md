# Contributing

Conventions for working in this repository, and what the gate checks
before a change lands. `README.md` covers day-to-day run, build, and test
commands; this page covers how the code is shaped and proven.

## The gate

`npm run verify` is the single quality gate: format and lint through
Biome, a TypeScript typecheck of the app and of `tools/` separately, the
vitest suite, a handful of small repository checks including this
documentation's link check, `cargo fmt --check`, and `cargo clippy -- -D
warnings`. `tools/verify.ts` is the source of truth for the exact lane
list; every lane runs concurrently, and every failure is reported, not
just the first. CI runs the identical command on every pull request.

## Module layout

There is exactly one path alias, `@/*` resolving to `./src/*`, defined
once in `tsconfig.json`'s own `paths`. Every internal package is reached
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

The app project includes `@types/node`, through `tsconfig.json`'s
`"types": ["node"]`, on purpose rather than by oversight.
`reducedMotion.spec.tsx` is the one file under `src/` that genuinely
needs Node's `fs`/`path`/`url` declarations, to walk the tree for a
no-literal-transition-duration check, and TypeScript's `types`
restriction applies to the whole program, not to one file, so there is
no narrower way to grant it. This does not conflict with the DOM types
the app otherwise relies on. If a future `@types/node` upgrade
reintroduces a clash between Node's and DOM's `setTimeout`/`setInterval`
return types, a typecheck failure across otherwise unrelated files is
the first place to look.

## `tools/`

`tools/` is TypeScript that Node runs directly, with no build step and no
loader flag, relying on Node's built-in type stripping. It has its own
`tools/tsconfig.json`, deliberately not referenced from or referencing
the app's `tsconfig.json`, so the two type universes cannot mix. Its
`erasableSyntaxOnly` setting means a script that drifts into a construct
Node's stripper cannot erase fails `npm run typecheck:tools` instead of
only at runtime.

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
