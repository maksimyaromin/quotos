---
id: note.rejected-ipc-payload-fails-silently
type: note
state: active
kind: fact
title: A rejected IPC payload fails silently and takes the whole command with it
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

A Tauri command whose payload fails to deserialize is never entered,
and the whole payload fails on one bad field. Nothing reaches the
command, nothing on screen changes, and the only trace is a rejected
promise on the frontend. The symptom is a surface frozen on whatever it
last drew, which looks like a rendering bug rather than like a contract
that no longer matches.

The instance that established this cost the whole menu bar. Serde's
`rename_all` on an enum renames its *variants*, not the fields inside
them; `rename_all_fields` is the attribute that reaches the fields. A
chip segment's `groupId` was therefore refused in the only spelling the
frontend ever sends, one rejected field failed the entire `segments`
array, and `set_status_item_state` was never entered at all — so from
the moment anyone made their first pin group the menu bar stopped
updating entirely, stuck on its startup paint with no chips, no figures
and the gauge at the 0% it boots with.

Two habits follow, and both are cheap:

- A test of a cross-boundary type deserializes the sender's own literal
  payload, in the sender's own spelling, rather than one written in the
  receiving language's spelling. A test written in Rust's spelling
  passes against exactly the bug it is there to catch. `shell.rs`'s
  `StatusItemSegmentDto` tests are written that way now.
- An `invoke` whose repaint is refused logs the rejection rather than
  dying as an unhandled promise rejection, so a broken contract says
  something instead of going quiet.

docs/status-item-rendering.md's "Crossing the IPC boundary" carries the
serde detail for this particular payload. The field in question is the
one that makes a group's chip clickable at all, under "A pin group
folds from its chip in the menu bar, not from the panel"
(decision.group-folds-from-its-chip).
