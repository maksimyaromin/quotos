Use `LimitWindow` for each entry in a subscription's expanded detail list. The list is variable — 1 to 8+ windows — so never assume a fixed "session / weekly" pair.

```jsx
<LimitWindow name="Session" remaining={62} resetLabel="resets in 3h" />
<LimitWindow name="Weekly (Opus 4)" remaining={8} resetLabel="resets Mon" scope="Opus 4" />
<LimitWindow name="Code review" />  // no percentage → shows "—", no bar
```

Names are passed through verbatim from the provider (possibly another language). Missing `remaining` renders "—" with no bar; missing `resetLabel` is simply omitted.
