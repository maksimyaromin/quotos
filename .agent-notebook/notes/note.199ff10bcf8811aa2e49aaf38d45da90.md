---
id: note.199ff10bcf8811aa2e49aaf38d45da90
type: note
state: active
kind: fact
title: The repository checks only see tracked files
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

Every repository check in `tools/checks/` — hidden characters, spec file
suffix, doc links, kebab case, comment length, provider name leak —
enumerates its inputs from `git ls-files`. A file that is not yet in the
index does not exist as far as they are concerned.

So a local `npm run verify` on a change that adds files is not the same
run CI performs. The new file passes locally by being invisible and
fails in CI, where it has been committed, with a failure that looks
unrelated to anything just written. Naming rules are the usual victim,
since they are the checks a brand new path is most likely to break.

Stage new files with `git add` before trusting a local verify. The other
lanes — format, lint, typecheck, test, cargo — read the working tree and
do not share this blind spot, which is what makes it easy to miss: most
of the gate sees the file and a handful of checks do not.
