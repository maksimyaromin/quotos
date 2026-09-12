---
name: anb
description: Recall and maintain project memory in repositories using agent-notebook. Use for continuing work, recording decisions or domain knowledge, and applying personal practices across sessions.
metadata:
  managed-by: anb
---

# anb

Keep the context that makes the next decision easier. The notebook connects work, domain knowledge and personal practices; it does not replace the project's tracker, documentation or source code. Use the CLI for notebook changes. Plain Markdown remains readable without this skill.

## Recall before acting

Run `anb recall` at the start of a session, unless a hook has already supplied it. It brings together current work, shared project knowledge and your private practices, with their audiences labelled. Open `.agents/anb.md` if present for the project's workflow extensions; do not copy or fork this installed skill to customize it.

Follow the user's subject. `anb recall --for <id>` prioritizes a record's context; `anb recall "phrase"` searches knowledge across all three audiences. Read the relevant records with `show`, using `--all` when the reply reports omitted content. Read referenced source material when the decision depends on it. A remembered conclusion is evidence with a scope, not permission to ignore the current request, code or source.

## Find the work before starting

Creating work does not assign it. A named request needs three steps:

1. Find the intended result. Search with `anb list --match "<words>" --team`, using the domain's vocabulary, and read plausible matches and their origins. An idea may have several deliveries; ask if the intended one remains ambiguous. Use `list --for <id> --team --all --archive` to inspect its composition, including completed, unassigned and other people's work. Dependencies and contextual links are not constituent work.
2. Agree the responsibility before changing assignments or state. For a whole result, any unfinished part assigned to another person requires a coordination question. Ask whether to coordinate the result or take a separate part; keep assignments and state unchanged until the answer. A request for one specific Task does not claim its siblings. Joining another session's Task also needs agreement. An explicit collaboration instruction can supply that agreement; a generic request to begin the result cannot.
3. Start the agreed work. For the whole result, start its Task, inspect `ready --for <id> --team` and explicitly start the chosen child; `start --next` would otherwise resume the active parent. The parent records accountability for the outcome, while each child keeps its own assignment and the session focuses on the step being executed. Do not start every child or assign every related record. Close the parent only after its required parts and overall acceptance are verified.

`anb start` resumes this session's focus when the host supplies a session identity. `anb start <id>` starts the named Task, and `anb start --next` takes eligible work. For the next part of a larger task, use `anb start --next --for <hub>`. Without an unambiguous focus, inspect `status` and ask which active Task the user means. Do not silently choose the most recently updated Task.

A session has one focus; a person may have several sessions and active Tasks. Switching this session does not put another session's work on hold. A hold means the work is waiting for something, not that an agent changed its attention. If this session already has active, unheld work within the requested scope, `start --next` resumes it; finish or explicitly switch that focus before taking more. An explicit `--for` can select a different scope.

## Choose what deserves memory

Save information when it changes future action and would otherwise be expensive or unreliable to recover. Search before creating a second account of the same fact. Keep the smallest record that explains the conclusion, its scope and the evidence or source behind it.

| What must survive | Where it belongs |
|---|---|
| Current result, failed attempt or next action | A comment on the relevant record |
| Work with a checkable outcome | A Task |
| An unresolved choice that blocks or changes future work | A Question |
| A settled constraint or choice, with its reason | A Decision |
| Reusable evidence, vocabulary, a model or a procedure | A Note |
| A canonical ticket, document or web source | A link plus the useful local conclusion |

A small request can be one Task. A brief read or explanation needs no Task. Create an idea Note when a proposal must outlive individual deliveries; a spec when maintained acceptance criteria need their own home; an investigation Task when research itself has a deliverable. None is a compulsory stage.

Use Note kinds to help retrieval: `fact`, `term`, `model`, `guide`, `idea` or `spec`. Decision kinds distinguish a standing `rule`, a design `shape` and an agreed exception `drift`. A Note's active state means it is maintained, not that its proposal is approved. Preserve uncertainty explicitly.

Progress is episodic: append what happened and what it changes. Reusable knowledge is consolidated: update the maintained explanation when evidence supports it. A repeated observation is not automatically a rule. When evidence contradicts a record, inspect the source and scope, then clarify, supersede or retire it; do not delete history merely because it is old. There are no invented confidence scores or automatic forgetting schedules.

Before consolidating, read the existing explanation and relevant alternatives alongside the new evidence. Integrate the supported change without discarding still-valid constraints. A new observation can refine an established model; a different ruling needs explicit supersession. Review knowledge when evidence, scope or a user request calls for it, not merely because it was retrieved.

Write for a reader who has no notebook tool or skill. Give the record a searchable title in the project's vocabulary. Open its body with the subject and useful conclusion, then include the scope, reason and source needed to act correctly. Name an important related record in prose as well as linking its id; an opaque id is not an explanation. Mark proposals, exceptions and superseded claims explicitly. Avoid “as discussed,” unexplained abbreviations and conclusions recoverable only from a chat transcript. Keep maintained knowledge separate from the chronological log of attempts.

## Choose the audience

The default notebook is shared project memory. Authorship records who contributed; it does not make a project rule private. Shared knowledge is recalled regardless of who wrote it. Work views share the notebook's configured `scope`: `mine` follows the current person's assignments, while the default `team` shows everyone's work. Explicit audience flags override that default for the current request.

Use `--personal` for this person's practices in this project and `--global` for practices across projects. Both stay outside the repository and hold Notes and Decisions, not Tasks or Questions. For “work this way for me here,” choose personal; for “always do this for me,” choose global. A team rule belongs in the project only when its team-wide authority is established. Ask if that distinction would materially change the instruction.

Record only the reusable instruction and its scope. Do not copy credentials, private customer evidence or personal details into shared memory. Private sources remain labelled during recall; they do not silently override a project requirement. If practices conflict, explain the relevant constraint instead of resolving it by audience rank.

Shared typed links resolve within the shared notebook. A colleague's `check` must not depend on private files. Link external systems with their canonical URL or project-relative path, for example `--link "doc <url>"`; retain provenance without copying the whole source. A notebook record does not authorize edits or messages in the linked system.

## Carry out the user's intent

Before an unfamiliar operation, read `anb <command> --help`. Use the returned id rather than reconstructing it from the title. Set `--via` to your actual agent tool on additions and attributed comments or outcomes; leave the accountable person's identity to the host.

| The user asks | Operation |
|---|---|
| “Change that wording” | `edit <id> --title "…"` or `edit <id> --body-file <path>` |
| “Add this finding” | `comment <id> --body "…"` |
| “Assign it to Grace” | `edit <id> --taken-by Grace` |
| “Put it back in the shared queue” | `edit <id> --clear taken-by` |
| “Ask Grace” | A Question with `--to Grace`; do not send an external message without authorization |
| “What is on my list?” | `status --mine`, or `ready --mine` for eligible work |
| “See what the team is doing” | `status --team`, or `ready --for <hub> --team` |
| “What is Grace doing?” | `status --by Grace` |
| “These need to happen first” | `block <task> <prerequisite>` |
| “This is waiting on an answer” | `hold <id> --reason "…"`; use `unhold` when it can resume |
| “It is done” | Verify, then `close <id> --body "Result, evidence and limits."` |
| “That work is no longer needed” | `close <id> --reason "…"` |
| “This knowledge no longer applies” | `retire <id> --body "What changed and where to look now."` |

Body flags have the same meaning on add, edit, comment and outcome commands: `--body` supplies text; `--body-file <path>` reads a file; `--body-file -` reads standard input. Editing a body replaces it, so read the existing record first and preserve unrelated content. Comments append without replacing it.

Make a Task's result and completion evidence clear. Connect records only for a reason: `--from` identifies their origin, `block` expresses a prerequisite, a bare id in prose supplies context, and `--link "<kind> <target>"` declares a typed relationship. Related work is not necessarily blocked work. For a larger outcome, a hub Task can have children created `--from` it; block the hub on deliverables whose completion it actually requires.

Do not take a colleague's Task just because you can edit its assignment. An explicit reassignment request authorizes that change; otherwise choose your own work or the untaken queue. Use `start <id> --join` only for intentional collaboration on the same Task.

## Keep a shared domain language

Build models from concrete scenarios and check them against the system. Explain who owns state, which changes must agree, what is allowed and what crosses a context boundary. Separate current behavior from proposals. A class name alone does not establish a domain concept.

Keep local definitions in the model; extract a term when independent lookup is useful. Where roles or contexts give a word different meanings, explain the translation rather than imposing one misleading definition. Link models to governing Decisions and relevant Tasks. Product, design, support and engineering can contribute scenarios, constraints and evidence to the same model without adopting an engineering-only workflow.

Keep each maintained claim in one place. Summarize the part needed for current work and link to its canonical source. A tracker still owns delivery commitments; documentation still owns published explanations; the notebook remembers how those sources affect this work.

## Verify and leave a continuation

Close work only after verifying its promised outcome. A concise outcome in the Task is enough; attach a separate report only when its length or reuse warrants one. Use `submit --to <name>` when acceptance belongs to someone else. Close answered Questions with `--resolved-by <id>` or `--reason`. Archive finished records when they no longer belong in the working set; keep reusable knowledge live.

Before leaving unfinished work, comment with the result, evidence and next concrete action. Run `anb check` after notebook changes and resolve relevant findings. A structured refusal supplies a code, cause and recovery action; correct the cause before retrying. If a write's result is uncertain, inspect the record before repeating creation. Commit only within the user's sharing policy and authorization.

The default replies use TOON, a standard compact representation of the same data returned by `--json`. Counts describe the total, omissions are explicit, and `more` gives the command that expands the same read. Recall reports omissions per audience in `sources`; inspect missing practices or context before relying on an incomplete read. A failed read is not an empty notebook.

Read [commands](references/commands.md) for the full surface, [the worked session](references/session.md) for literal replies, or [refusals](references/refusals.md) when a recovery instruction needs context. Load only the reference relevant to the current operation.
