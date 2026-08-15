The core Quotos row. One per subscription in the panel. Drives its layout from whether data exists: a row with a `used` number shows the "% used" headline + capacity bar; one without (`idle` / `connecting` / `broken`, or a good read with nothing to report) shows its `reason` message and an inline action. Age (`lastRead`) is always visible; `behind` turns the numeral and the read time amber — stale data is never presented as current.

```jsx
<SubscriptionRow
  label="Claude Max" provider="Anthropic" account="Personal"
  state="working" used={62} resetLabel="resets in 3h" lastRead="2 min ago"
  expanded={open} onToggleExpand={() => setOpen(!open)}
  windows={[
    { id: "five_hour", name: "Session", used: 62, resetLabel: "resets in 3h" },
    { id: "weekly_scoped:opus", name: "Weekly", used: 41, resetLabel: "resets Mon", scope: "Opus 4" },
  ]}
/>

<SubscriptionRow label="Claude Team" state="broken"
  reason="Sign-in expired. Reconnect to resume reading."
  actionLabel="Reconnect" onAction={fix} lastRead="1h ago" />
```

Compose a list of these inside `Panel`. The expander appears only when `windows` is non-empty. `used` is % *consumed* — the whole surface says "used", never "left". The headline and bar tint from `severity` (the provider-computed worst of *every* window), not from the headline number's own magnitude.
