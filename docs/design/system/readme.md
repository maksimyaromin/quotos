# Quotos — Design System

Quotos is a **macOS menu bar utility** that answers one question at a glance:
*on which of my AI subscriptions can I keep working right now?* It lives as a
tray icon with a compact popover; the whole product is local, with no account
or server.

This design system defines the visual language, tokens, components, and
full-screen recreations for that product. It is authored from a written brief
(no prior code or brand existed), so the identity below is proposed, not
inherited — treat it as a strong first direction to iterate on.

## Sources

- **Brief:** `quotos/design/brief.md` (mounted codebase, read-only) — the
  authoritative spec of what the product is, the data model, and every state
  the UI must render. Makes no visual decisions on purpose.
- **Product readme:** `quotos/README.md` — one-paragraph "why".
- No Figma, no existing components, no logo were provided. There is **no
  codebase yet** — this project is the first visual pass.

## The product in one screen

- **Menu bar icon** — always present, monochrome (macOS template style).
  Optionally shows pinned windows' consumed figures beside it.
- **Panel (popover)** — opens on click, ~332px wide. One scannable row per
  subscription: a big "% used" numeral, a thin capacity bar, the age of the
  last read, and a state indicator. Rows expand to show every limit window.
- **Add-subscription flow** — the central flow, not a settings afterthought:
  scan the machine → pick provider → pick connection method → verify by reading
  once → confirm. Failure offers another method, never a dead end.

## Product principles (from the brief, and how the design answers them)

1. **Never present an old number as current.** Age is always visible. When data
   is stale ("Behind"), the numeral is dimmed and stamped with its read time —
   it never looks live.
2. **Reassurance, not anxiety.** The common case ("plenty left, carry on") is
   deliberately quiet: neutral row, calm teal bar, no badge. Color and weight
   are spent only on the rare case that needs attention.
3. **Unknown quantity, unknown vocabulary.** Any number of subscriptions, any
   number of limit windows each, named in the provider's own words (possibly a
   name we've never seen, possibly a different language). The detail list is a
   variable list, never a fixed "session / weekly" layout.
4. **Independent failure.** State is per-subscription; one broken account never
   blanks the others. There is no global error screen.

### Challenges to the brief (decisions taken)

- **Headline is "% used", not "% left."** The first visual pass framed the
  headline as remaining capacity; the build deliberately inverted every
  percent surface to consumed — a fuller bar always means more used, so the
  numeral, the bar, and the tray digits can never disagree about direction.
- **Menu bar stays monochrome and shows at most a couple of pinned figures.**
  Color in the menu bar would violate macOS convention and "don't shout"; the
  pinned figure only gains a warning tint when it actually needs attention.
- **Healthy is colorless.** A green "all good" badge would make the common case
  loud. Healthy rows carry no status color at all; the teal bar is the only
  brand note.

## Visual foundations

**Appearance.** Dark is the hero appearance; a light appearance mirrors every
token (`[data-appearance="light"]`). The popover reads as a floating vibrancy
surface — a large soft drop shadow plus a 0.5px hairline rim over a blurred
backdrop.

**Color.** A cool graphite neutral ramp (`--graphite-0…9`) carries almost all
surfaces and text. A single **muted teal** accent (`--teal`, `#3f8b7e` dark /
`#1f7c6f` light) is the brand note and the "healthy capacity" fill —
deliberately low-saturation so it never glows. Semantic hues are muted by
design: amber (`--amber`) for stale/behind, coral-red (`--red`) for broken,
slate-blue (`--blue`) for informational accents. Never more than
one accent plus at most one semantic hue on screen at once. See
`tokens/colors.css`.

**Capacity color logic.** The bar fill encodes how much is used: teal while
comfortable, amber from 75% used, red from 90%. That is the only place color
changes meaning by value.

**Type.** UI text is **Inter** (variable, self-hosted); every number that
matters is set in **MonoLisa** (static weights 400/500/600/700, self-hosted) so
digits align in a list and read like data. Both font families were provided by
the owner and ship with the system (`assets/fonts/`, declared in
`tokens/fonts.css`). Sizes are compact for a popover (11–16px working range;
13px body). The headline "% used" is 30px mono; the verify-result hero is 44px.
See `tokens/typography.css`.

**Spacing & radii.** A tight 2px-based scale (`--space-*`) — density is a
feature here, whitespace is earned. Radii follow macOS: 5–6px controls, ~12px
popover shell, ~4px chips. See `tokens/spacing.css`.

**Elevation & motion.** Depth comes from a single float (the popover), not
stacked shadows; inner surfaces are flat. Motion is quick and flat — short
fades and 140–200ms eases, no bounce, no overshoot. A "reading" refresh shows
an indeterminate shimmer on the bar rather than blanking the number. See
`tokens/elevation.css`.

**Borders & surfaces.** Hairlines are semi-transparent white (dark) / black
(light) at 6/10/16% — `--border-subtle/default/strong`. Cards and menus are a
single step up in surface (`--bg-elevated`) with a hairline, no heavy shadow.

**Hover / press.** Rows and controls lighten one surface step on hover
(`--bg-row-hover`); primary buttons darken/brighten the accent; press states
use a brief opacity dip and a 1px settle, never a big scale.

## Content fundamentals

Voice is **plain, quiet, and factual** — a good macOS system utility, not a
marketing surface.

- **Sentence case everywhere.** No Title Case buttons, no ALL CAPS except tiny
  tracked meta labels ("LAST READ").
- **Short, literal labels.** "Add subscription", "Refresh", "Try another way".
  Actions are verbs; nothing is cute.
- **Address the person as "you", refer to the app as "Quotos" sparingly.** The
  app rarely refers to itself; it just states facts ("Last read 2 min ago").
- **State copy is calm and specific.** "Behind" states say what's held and when
  it was read; "Broken" states say the reason and the one thing you can do.
  Never "Oops" or exclamation marks.
- **Provider vocabulary is passed through verbatim.** Window names ("Session",
  "Weekly", "Opus-scoped") are quoted from the provider, never rewritten — even
  in another language.
- **Numbers carry units, time is relative.** "92% used", "resets in 3h",
  "2 min ago". Absolute timestamps appear on hover / in detail.
- **No emoji.** Status is carried by a small colored dot and a word, not an
  emoji. Iconography is line icons (see below).

## Iconography

- **Set:** [Lucide](https://lucide.dev) (ISC license) — a restrained 1.5px
  stroke line set that matches macOS toolbar conventions. Loaded from CDN
  (`unpkg.com/lucide@latest`) in cards and UI kits; documented as a substitute
  because no icon set was provided. Swap for a licensed set if desired.
- **Usage:** icons are monochrome, inherit `currentColor`, and sit at 14–16px in
  controls, 13px inline. Common glyphs: `refresh-cw`, `plus`, `settings`,
  `chevron-down`, `search`, `check`, `alert-triangle`, `x`, `pin`.
- **Mark:** a proposed original **capacity-gauge** mark (a ring with a swept
  arc), designed for this product at the owner's request — not derived from any
  existing logo. Lives as a monochrome macOS template glyph in the tray
  (`assets/menubar-glyph.svg`) and as an accent app icon
  (`assets/app-icon.svg`). When nothing is pinned, the tray shows this glyph
  alone; pinned figures sit to its right.
- **No emoji, no unicode-symbol icons.** Status uses colored dots + words.

## Index / manifest

Root:
- `styles.css` — global entry point (import this).
- `tokens/` — `fonts.css`, `colors.css`, `typography.css`, `spacing.css`,
  `elevation.css`.
- `guidelines/` — foundation specimen cards (Type, Colors, Spacing, Elevation).
- `assets/` — the placeholder menu-bar glyph.
- `SKILL.md` — Agent-Skill wrapper.
- `thumbnail.html` — project tile.

Components (`components/<group>/`):
- `controls/` — Button, IconButton, TextField
- `indicators/` — CapacityBar, StatusDot, Badge
- `subscription/` — SubscriptionRow, LimitWindow
- `shell/` — Panel, MenuBarTile

UI kit (`ui_kits/quotos/`):
- `index.html` — the interactive click-through (panel → expand → add flow →
  states → light/dark).

## CAVEATS

- **Mark is a proposal.** The capacity-gauge glyph and app icon were designed
  for this product at the owner's request; they are original, not a reconstruction
  of any existing logo. Iterate freely.
- **MonoLisa weights are mapped by best guess.** The provided files were numbered,
  not named; 400/500/600/700 map to files `6/8/10/12`. If a weight looks off,
  point the `@font-face` in `tokens/fonts.css` at a different file.
- **Icons are Lucide via CDN**, chosen as a close match — swap if you have a set.
