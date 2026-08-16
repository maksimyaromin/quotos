Use `Panel` as the popover container for the whole product view. Put `IconButton`s in `headerActions` and the primary "Add subscription" button in `footer`; the body holds `SubscriptionRow`s and scrolls at `maxBodyHeight`.

```jsx
<Panel
  title="Quotos"
  headerActions={<><IconButton label="Refresh"><RefreshIcon/></IconButton>
                   <IconButton label="Settings"><SettingsIcon/></IconButton></>}
  footer={<Button variant="ghost" size="sm" icon={<PlusIcon/>}>Add subscription</Button>}
>
  {subs.map(s => <SubscriptionRow key={s.id} {...s} />)}
</Panel>
```

The beak points up at the status item. Set `beak={false}` to hide it when previewing the panel detached.
