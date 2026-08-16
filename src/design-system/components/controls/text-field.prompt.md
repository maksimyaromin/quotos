Use `TextField` for typed input in the add-subscription flow: pasting a key, with `mono` set, or naming an account. macOS small-control height, teal focus ring.

```jsx
<TextField label="API key" mono placeholder="sk-ant-…" value={key} onChange={e => setKey(e.target.value)} invalid={failed} />
```

Set `mono` for keys/tokens so characters align. `invalid` shows a red border for a failed field.
