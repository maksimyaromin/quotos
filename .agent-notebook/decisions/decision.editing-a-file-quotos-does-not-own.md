---
id: decision.editing-a-file-quotos-does-not-own
type: decision
state: active
kind: rule
title: "Editing a file Quotos does not own: merge, refuse on garbage, store no state"
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

`settings.json` under a Claude Code config directory belongs to another
application and to whoever edits it by hand. Quotos writes it under a
fixed discipline, and any future integration that has to touch a file
it does not own inherits the same one.

Read, merge, write. Every key that is not ours is preserved, including
the `padding` and `refreshInterval` of a `statusLine` entry being
wrapped. A file that does not parse is refused and left exactly as it
is, on the grounds that a file Quotos cannot understand is one it
certainly cannot rewrite. The write itself is a temp file in the same
directory followed by a rename, so an interrupted write cannot leave
the other application with half a config.

Nothing about the integration's state is stored on our side. Enabled
means one thing, computed fresh every time: the command in
`settings.json` is the command Quotos would generate for that account.
A status line the user changed by hand through the other app's own
interface simply reads as off, with nothing to reconcile and no
conflict state to design a recovery for. Disabling restores the
previous value exactly, or clears the key when there was none, and
removes the artefacts Quotos left beside it.

And nothing Quotos installs into another application's config points at
the Quotos binary. What goes in is a generated, self-contained POSIX
`sh` script with no arguments, no `jq` and no path back to us, so
moving the app, replacing it or deleting it leaves the other
application running something that still works. An earlier version
copied the binary and pointed at the copy, which is the mistake this
rule exists to prevent; `migrate_legacy` is the one-time cleanup that
still carries its cost.

docs/claude-provider.md's "The statusline feed" describes the script,
the feed file and the migration in full.
