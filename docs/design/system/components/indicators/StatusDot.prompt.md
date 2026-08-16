Use `StatusDot` to show a subscription's state compactly, paired with a word (never alone). Six states map to muted colors; `connecting` / `reading` pulse.

```jsx
<StatusDot state="behind" /> <span>Behind</span>
```

`working` is calm teal (not a shouting green) so the common case stays quiet. `idle` (not connected) is a hollow ring. See `SubscriptionState` for the full set.
