Use `CapacityBar` for any "how much is left" reading — the row headline and each limit window. `remaining` (0–100) drives both width and color: teal >25, amber ≤25, red ≤10.

```jsx
<CapacityBar remaining={8} />           // nearly out → red
<CapacityBar remaining={62} reading />  // refreshing → shimmer over held value
<CapacityBar remaining={40} stale />    // behind → dimmed fill
```

`reading` shows an indeterminate shimmer (never blanks the number); `stale` dims the fill for behind data. `capacityColor(remaining)` is exported to tint a numeral the same way.
