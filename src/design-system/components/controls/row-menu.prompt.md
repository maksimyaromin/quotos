`RowMenu` is the overflow menu a row hangs off its "…" button. It is design system plumbing, not a component to reach for directly: `SubscriptionRow` uses it so every menu in the panel places, styles and keyboard-navigates identically.

```jsx
const menu = useRowMenu(open)
<button ref={menu.triggerRef} aria-haspopup="menu" aria-expanded={open} data-quotos-menu-scope="true" />
{open ? (
  <RowMenu anchor={menu} label="Subscription actions">
    <MenuItem onClick={rename}>Rename</MenuItem>
    <MenuSeparator />
    <MenuItem danger onClick={remove}>Stop tracking</MenuItem>
  </RowMenu>
) : null}
```

The menu is `position: fixed` against the viewport, flipping above its trigger rather than off the bottom of the window. Put `anchor.onKeyDown` on an ancestor of both trigger and menu so the arrow keys work from either. Whoever owns the open state closes it; `data-quotos-menu-scope` is what the panel's click-away handler matches on.
