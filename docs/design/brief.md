# Quotos design brief

For whoever designs the interface. This document describes **what the product
is, what data exists, and every state that must be drawn**. It deliberately
makes no visual decisions: layout, typography, colour, motion, and component
choices are yours.

---

## 1. What it is

A macOS menu bar utility that answers one question at a glance:

> **On which of my subscriptions can I keep working right now?**

Not a Claude usage widget. A capacity dashboard for **all** AI subscriptions a
person owns, whoever issued them.

**Moment of use.** An agent is working. The owner wants to know, without
breaking flow, whether there is enough allowance left to keep going, on this
subscription or another one.

**Emotional target.** Reassurance, not anxiety. The common case is "plenty
left, carry on". The interface should make the common case invisible and the
rare case obvious.

---

## 2. The core entity: a subscription

Everything in the product is built around one object. Design for **an unknown
number of them**: one person has two, another has ten, from different
providers.

A subscription has:

| Property | Meaning | Notes for design |
|---|---|---|
| **Label** | how it is named to the person | comes from the provider, renameable; two subscriptions of the same person can look confusingly similar |
| **Provider** | who issued it | a person may hold several from the same provider |
| **Headline number** | the one number that answers "can I keep working" | the most consumed of its currently active limits |
| **Details** | every other limit it has | count and names vary per subscription and change over time |
| **State** | see section 4 | each subscription has its own; one failing must not break the others |
| **Last read** | date and time of the last successful read | always shown, never hidden |
| **Pinned** | whether it appears next to the menu bar icon | owner's choice, several may be pinned |

### The one rule that is not negotiable

**Never present an old number as current.** The product exists so someone can
decide whether to keep working. A wrong number is worse than no number. Age is
always visible.

---

## 3. What data actually exists

Each subscription returns a **list of limit windows**. The list is not fixed:
its length, names and kinds differ per subscription and change over time. Design
for a variable list, not for a known set of rows.

Each window may carry:

- **a name**: provider's own wording, e.g. a session limit, a weekly limit, a
  limit scoped to one model
- **percent consumed**: 0 to 100; may be absent
- **a reset time**: when it refills; may be absent
- **a scope**: some windows apply only to part of the subscription, e.g. one
  model

Typical shapes to design against:

- a subscription with two windows, both with percentages and reset times
- a subscription with six windows, two of them scoped to specific models
- a window with a name but no percentage
- a window whose name the product has never seen before. It must still be
  shown, using the provider's own wording
- a subscription whose provider reports nothing useful at all

**Consequence:** a fixed row layout of "session / weekly" will break. The detail
view must render an arbitrary list gracefully at 1, 3 and 8 windows.

---

## 4. States: the heart of the design work

Each subscription is independently in one of these. All of them will be seen in
normal use.

| State | What happened | What it must communicate |
|---|---|---|
| **Not connected** | added but never successfully read | needs an action to finish setup |
| **Connecting** | verifying a new connection | in progress, may take a few seconds |
| **Working** | data is current | the numbers, plainly |
| **Reading** | a refresh is in flight | previous numbers remain visible; do not blank them |
| **Behind** | refresh failed, older data is held | the numbers **with their date and time**, clearly not fresh |
| **Repairing** | a recoverable problem is being fixed automatically | brief, self-resolving, usually a couple of seconds |
| **Broken** | cannot be read at all | the reason and what the person can do |
| **Waiting on limits** | the provider refused a too-frequent read | not an error; it will resolve by itself |

Mixed states are the normal case: three subscriptions working, one behind, one
broken. The panel must stay readable and calm in that mix.

### Timing to design against

- A normal read is fast, well under a second.
- A read that first has to repair a stale credential takes a couple of seconds.
- The very first read after opening the panel is the one most likely to be slow.

---

## 5. Surfaces to design

### 5.1 The menu bar icon

Always present. Shows product identity, and optionally the pinned
subscriptions' numbers beside it. Must survive:

- nothing pinned
- one pinned subscription
- several pinned subscriptions: menu bar space is scarce and shared with every
  other app
- pinned data that is behind or broken

### 5.2 The panel

Opens on click. Shows every subscription. Needs:

- a headline number per subscription, scannable in one pass
- access to the detail windows without cluttering the default view
- date and time of last read, per subscription
- a manual refresh
- a way to reach adding a subscription
- graceful behaviour at 1, 5 and 10+ subscriptions

While the panel is open it refreshes every two minutes, so the design must
handle values changing under the person's eyes without jarring them.

### 5.3 Adding a subscription

**This is the product's central flow, not a settings afterthought.** It is what
makes the utility work for anyone rather than for one person's setup.

The flow:

1. The product **scans the machine** and reports what it found, for example
   "found two subscriptions", with the option to add them.
2. If nothing is found, or the person wants something else, they pick a
   provider manually.
3. They pick **how to connect**. A provider may offer several methods, e.g.
   using the credentials of an already-installed client, or a key pasted by
   hand. The choice must be explainable without jargon.
4. The product **verifies** by reading once, and shows what came back so the
   person can confirm it is the right account.
5. Saved.

Failure inside this flow is expected and must be designed: verification fails,
and the person is offered another connection method rather than a dead end.

### 5.4 Notification

A system notification when a subscription approaches its limit. Fires once per
limit window so it never becomes noise. The threshold value, and whether it is
configurable, is your call.

---

## 6. Constraints the design must respect

- **macOS menu bar conventions.** This lives among system icons; it must not
  shout.
- **Unknown quantity.** Any number of subscriptions, any number of windows each.
- **Unknown vocabulary.** Window names come from providers and are not under our
  control. Text may be longer than expected and may be in English while the rest
  of the interface is not.
- **Independent failure.** Per-subscription state, never a single global error
  screen.
- **Age is always visible.** No exceptions, including for pinned values in the
  menu bar.
- **Refreshes happen in the background too**, so values can change between two
  openings of the panel with no interaction in between.

---

## 7. Deliberately left to design

- Everything visual: layout, colour, type, iconography, density, motion.
- How the headline number is expressed: percentage, bar, gauge, remaining vs
  consumed, time until reset.
- Whether detail windows are always visible, expandable, or on a second level.
- How several subscriptions are arranged: list, grouped by provider, or another
  structure entirely.
- What exactly appears next to the menu bar icon when several are pinned.
- The warning threshold and whether it is adjustable.
- Whether the first run has any onboarding beyond the add-subscription flow.

---

## 8. Out of scope for the first version

- Providers other than the first one. The interface must not assume a
  single provider anywhere, since more are coming.
- History, trends, charts over time.
- Anything requiring an account or a server. The product is local.
- Distribution and publishing.
