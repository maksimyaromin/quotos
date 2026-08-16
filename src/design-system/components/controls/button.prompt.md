Use `Button` for any labeled action. One `primary` per surface; everything else `secondary` or `ghost`. Labels are sentence-case verbs.

```jsx
<Button variant="primary" size="base" onClick={connect}>Add subscription</Button>
<Button variant="ghost" size="sm">Try another way</Button>
```

Variants:
- `primary`: teal fill, the single main action.
- `secondary`: default, hairline.
- `ghost`: text only, low emphasis.
- `danger`: red text, destructive.

Sizes: `sm` at 22px, `base` at 26px, macOS's own small control height, and `lg` at 30px. Pass `icon` for a 14px leading glyph and `disabled` to dim to 40%. `fullWidth` stretches it, which the sheet's confirm button uses.
