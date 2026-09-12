# Notebook practice for this project

How `.agent-notebook/` is kept here, on top of what the `anb` skill
already says. Read this before writing a record.

## Recall first

Run `anb recall` before starting substantive work on this codebase —
anything that changes behaviour, geometry, persistence or a boundary
between the two processes. Several of the rules in the notebook exist
because the obvious implementation is the wrong one, and they only help
if they are read before the code is written rather than after.

## Every record gets a stable, readable id

A record that earns a place in the notebook is created with an explicit
`anb add … --id <id>` in this project's own vocabulary:
`decision.group-folds-from-its-chip`, `note.checks-see-only-tracked-files`.
Never the random id the CLI allocates by default. The id is what other
records cite in their prose and what a future reader greps for, so it
has to say what the record is about; a 128-bit hex string says nothing
and makes every cross-reference unreadable. Title it the same way: a
sentence someone would search for, in the words this repository already
uses — mark, chip, figure, limit window, pin group, panel, status item.

## Write the record as part of the change

A change that settles a non-obvious design question, and a bug whose
cause generalises past its own fix, gets a Decision or a Note while that
work is being finished — in the same branch, not in a later cleanup pass.
A rule written a week after the fact loses the alternatives that were
tried and the measurements that ruled them out, which is the part that
was expensive to get.

Use the types as the skill defines them. This notebook is mostly
Decisions and Notes, because it holds domain knowledge and decision
history rather than a queue: a Decision for a settled constraint or
shape with its reason, a Note for reusable evidence, vocabulary or a
procedure. Reach for a Task only for work with a checkable outcome that
is actually open, and a Question only for a choice that is genuinely
unresolved.

## Do not restate the docs

`docs/` owns the published explanation and `AGENTS.md` owns the
orientation. The notebook holds what is true but written down nowhere
else: why a shape was chosen over the alternatives that were tried, what
a failure looked like before its cause was understood, and the rules
that span more than one page and so belong to no single one of them.
Summarise only as much as the record needs to stand on its own, then
name the canonical page. A record that could be replaced by a link is
not worth keeping.

Name a related record in prose as well as by id, so the connection still
reads for someone with the plain Markdown and no tooling.

## Record the software, never how it was made

This repository is public and the notebook is part of it. A record is a
settled technical fact with its rationale, written in the present tense,
as if the code had always been this way. It never narrates how the work
happened: no authorship or review attribution of any kind, no model or
vendor names as who wrote or checked something, no tooling or workflow
that surrounds the repository, no internal task or ticket identifiers,
and no count of how many attempts a thing took. "An earlier shape did X
and was replaced because Y" is the right way to keep what a rejected
attempt taught.

"Claude Code" appears only as what it is in this product: the
subscription Quotos tracks and the CLI it reads a statusline feed from.
It is never a byline.

Re-read anything written into the notebook against this section before
committing it.

## After changing the notebook

Run `anb check` and resolve what it reports. It is part of finishing
the change, like `npm run verify`.
