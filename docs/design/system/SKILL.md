---
name: quotos-design
description: Use this skill to generate well-branded interfaces and assets for Quotos, either for production or throwaway prototypes/mocks/etc. Contains essential design guidelines, colors, type, fonts, assets, and UI kit components for prototyping.
user-invocable: true
---

Read the README.md file within this skill, and explore the other available files.
If creating visual artifacts (slides, mocks, throwaway prototypes, etc), copy assets out and create static HTML files for the user to view. If working on production code, you can copy assets and read the rules here to become an expert in designing with this brand.
If the user invokes this skill without any other guidance, ask them what they want to build or design, ask some questions, and act as an expert designer who outputs HTML artifacts _or_ production code, depending on the need.

## Quick map

- `readme.md` — the full design guide: product context, content fundamentals, visual foundations, iconography, and the file index. Start here.
- `styles.css` — the global entry point; link this one file to get every token and the webfont import.
- `tokens/` — `colors.css`, `typography.css`, `spacing.css`, `elevation.css`, `fonts.css`.
- `components/` — reusable React primitives (`controls/`, `indicators/`, `subscription/`, `shell/`). Each has a `.jsx`, a `.d.ts` props contract, and a `.prompt.md` usage note.
- `ui_kits/quotos/` — an interactive click-through of the whole product; the best reference for how the pieces compose.
- `guidelines/` — foundation specimen cards.
- `assets/` — the placeholder menu-bar glyph (Quotos has no logo yet).

## Working rules of thumb

- Quiet by default: the common case ("plenty left") carries no status color. Spend color only on attention.
- Numbers are IBM Plex Mono, tabular; UI text is the macOS system font.
- One teal accent plus at most one semantic hue on screen at once.
- Age is always visible; never present stale data as current.
