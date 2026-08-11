Use `MenuBarTile` to represent Quotos in the macOS menu bar — the quota-ring glyph plus any pinned subscriptions' figures. Stays monochrome; a figure gains an amber/red tint or a small warning mark only when it needs attention (behind, broken, or low).

```jsx
<MenuBarTile pins={[{ remaining: 62, state: "working" }, { remaining: 8, state: "working" }]} onClick={open} />
<MenuBarTile pins={[{ state: "broken" }]} />          // shows a warning mark, not a stale number
<MenuBarTile showStrip={false} pins={[]} />           // bare glyph, e.g. inside docs
```

`showStrip` (default) frames it on a mock menu-bar with faint battery/clock neighbors to show scale. `QuotaGlyph` is exported for standalone use. This is a placeholder mark — Quotos has no logo yet.
