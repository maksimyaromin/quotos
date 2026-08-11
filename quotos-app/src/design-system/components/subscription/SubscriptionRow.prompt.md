The core Quotos row. One per subscription in the panel. Drives its entire layout from `state`: data-bearing states (`working`, `reading`, `behind`, `waiting`, `repairing`) show the "% left" headline + capacity bar; `idle` / `connecting` / `broken` show a message and an inline action. Age (`lastRead`) is always visible; `behind` dims the numeral and stamps the read time amber.

```jsx
<SubscriptionRow
  label="Claude Max" provider="Anthropic" account="Personal"
  state="working" remaining={62} resetLabel="resets in 3h" lastRead="2 min ago"
  pinned expanded={open} onToggleExpand={() => setOpen(!open)}
  windows={[
    { name: "Session", remaining: 62, resetLabel: "resets in 3h" },
    { name: "Weekly", remaining: 41, resetLabel: "resets Mon", scope: "Opus 4" },
  ]}
/>

<SubscriptionRow label="Claude Team" state="broken"
  reason="Sign-in expired. Reconnect to resume reading."
  actionLabel="Reconnect" onAction={fix} lastRead="1h ago" />
```

Compose a list of these inside `Panel`. The expander appears only when `windows` is non-empty. `remaining` is % LEFT (reassuring framing), computed as 100 − most-consumed limit.
