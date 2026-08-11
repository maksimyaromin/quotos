Use `Button` for any labeled action. One `primary` per surface; everything else `secondary` or `ghost`. Labels are sentence-case verbs.

```jsx
<Button variant="primary" size="base" onClick={connect}>Add subscription</Button>
<Button variant="ghost" size="sm">Try another way</Button>
```

Variants: `primary` (teal fill, the single main action), `secondary` (default, hairline), `ghost` (text only, low emphasis), `danger` (red text, destructive). Sizes: `sm` 22px, `base` 26px (macOS small control), `lg` 30px. Pass `icon` for a 14px leading glyph, `fullWidth` to stretch (the sheet's confirm button), `disabled` to dim to 40%.
