Use `IconButton` for compact toolbar actions with no text label: the panel header's refresh, settings, and add controls, a row's expand chevron, a sheet's close. Always pass `label`, which doubles as the tooltip and the accessibility label.

```jsx
<IconButton label="Refresh" onClick={refresh}><RefreshIcon /></IconButton>
<IconButton label="Pin to menu bar" active={pinned} onClick={togglePin}><PinIcon /></IconButton>
```

Monochrome, inherits `currentColor`, lightens its background on hover. `active` turns the glyph teal, as used for the pin toggle.
