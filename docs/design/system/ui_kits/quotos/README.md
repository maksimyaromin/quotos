# Quotos UI kit

An interactive, click-through recreation of the whole product: the macOS menu
bar tile and the popover panel, with every subscription state and the
add-subscription flow. It composes the design-system primitives from the
compiled bundle — it does not re-implement them.

## Run

Open `index.html`. It loads React + Babel + Lucide (CDN), the design-system
bundle (`../../_ds_bundle.js`), then `data.js` and `app.jsx`.

## Files

- `index.html` — page shell, script loading, scrollbar/appearance chrome.
- `data.js` — mock data: a mixed-state set of subscriptions and the
  add-subscription flow content. Plain globals, no build step.
- `app.jsx` — the prototype: desktop + menu-bar chrome, the popover, and the
  add-subscription wizard. Composes `Panel`, `SubscriptionRow`, `MenuBarTile`,
  `Button`, `IconButton`, `TextField`.

## What you can do

- **Toggle the panel** — click the quota ring in the menu bar.
- **See mixed states** — working (incl. a critical 8% row), behind (dimmed,
  stamped), waiting on limits, broken (with a reason + Reconnect), and a
  not-connected seat. One failing never blanks the others.
- **Expand a row** — reveals its variable window list (1, 2, or 3 windows,
  including a scoped and a foreign-language window name).
- **Refresh all** — rows shimmer ("reading") over their held numbers, then
  settle to fresh values; nothing blanks.
- **Pin / unpin** — hover a row, toggle the pin; the menu-bar figures update
  live (a low pin tints for attention, the rest stay monochrome).
- **Add a subscription** — scan → found accounts → provider → connection method
  → verify → confirm. Paste `sk-ant-ok` on the API-key method to succeed;
  anything else fails and offers another method.
- **Light / dark** — the sun/moon control in the header.

## Fidelity notes

This is a cosmetic recreation. Data is mocked; timers stand in for real reads;
"scanning the Mac" is simulated. Layout, tokens, and component behavior are the
real design-system ones.
