Use `MenuBarTile` to represent Quotos in the macOS menu bar: the brand mark plus any pinned subscriptions' figures. Figures stay monochrome; one gains an amber/red tint or a small warning mark only when it needs attention, whether behind, broken, or nearly spent.

```jsx
<MenuBarTile pins={[{ used: 38, state: "working" }, { used: 92, state: "working" }]} onClick={open} />
<MenuBarTile pins={[{ state: "broken" }]} />          // shows a warning mark, not a stale number
<MenuBarTile showStrip={false} pins={[]} />           // bare glyph, e.g. inside docs
```

`showStrip` (default) frames it on a mock menu-bar with faint battery/clock neighbors to show scale. `QuotaGlyph` is exported for standalone use and takes its own `used`: the two grey chevrons are spend rising and never move, while the peach one is the gauge, faint at full extent with its bright pass growing from the vertex up toward the arm-tips. `status_item_render.rs` is the drawing that ships; this one stands for it inside the design system.
