Use `Wordmark` wherever the product names itself in the interface — today that is `Panel`'s `title` on the main screen. It takes no props: it is the lockup in `docs/brand/` set as live text, without the mark, since the mark is already in the menu bar the panel hangs from.

```jsx
<Panel title={<Wordmark />}>…</Panel>
<Panel title="Subscriptions">…</Panel>   // a sub-screen names itself in words instead
```

`supolka(` and `)` take the base text colour, `quotos` the `--peach` brand accent, the trailing `│` the `--rosewater` one. Those two accents identify the product; they are never used for a severity, which teal, amber, red and blue already mean.
