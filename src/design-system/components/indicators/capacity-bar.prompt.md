Use `CapacityBar` for any "how much is used" reading: the row headline and each limit window. `used` runs from 0 to 100, the percent consumed, and always sets fill width: teal below 75, amber at 75 or more, red at 90 or more.

```jsx
<CapacityBar used={92} />                    // nearly spent → red
<CapacityBar used={38} reading />            // refreshing → shimmer over held value
<CapacityBar used={60} stale />              // behind → dimmed fill
<CapacityBar used={20} severity="warn" />    // headline bar: worst window colors it
```

Color comes from `severity` when given: the subscription's headline bar, colored by the worst of every window. Otherwise it comes from `used`'s own bracket, a single window's bar. `reading` shows an indeterminate shimmer and never blanks the number; `stale` dims the fill for behind data. `capacityColor(used)` is exported to tint a numeral the same way.
