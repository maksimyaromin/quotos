Use `LimitWindow` for each entry in a subscription's expanded detail list. The list is variable, from 1 to 8+ windows, so never assume a fixed "session / weekly" pair.

```jsx
<LimitWindow id="w1" name="Session" used={38} resetLabel="resets in 3h" />
<LimitWindow id="w2" name="Weekly (Opus 4)" used={92} resetLabel="resets Mon" scope="Opus 4" />
<LimitWindow name="Code review" />  // no percentage → shows "—", no bar
```

Names are passed through verbatim from the provider, possibly in another language. Missing `used` renders "—" with no bar; missing `resetLabel` is simply omitted. Each row leads with its pin button: `pinned` reflects whether this window's figure is in the menu bar, and `onTogglePin` is called with the row's `id`.
