# The Claude provider

Everything here is specific to the Claude Code adapter,
`src-tauri/src/providers/claude.rs` and `src/providers/claude/`. See
[architecture.md](architecture.md) for the generic provider seam this
adapter plugs into.

## Reading the account

Quotos reads a Keychain-backed OAuth token and calls the undocumented but
first-party `/api/oauth/usage` and `/api/oauth/profile` endpoints. That
endpoint is rate-limited by Claude's own servers, shared with Claude Code
itself; Quotos never models that limit locally, it just reads the
provider's own `Retry-After` on a 429 and backs off. See "Refresh
scheduling and pausing when nobody is looking" in
[architecture.md](architecture.md).

The Keychain item's own decrypt and export ACL trusts exactly
`/usr/bin/security` with `promptSelector=0`, so the `security
find-generic-password -w` call this app shells out to is authorized and
never raises a sign-in prompt. `QUOTOS_DEBUG_READS=1` traces the
credential and read decisions, never a token, to standard error.

`fetch_usage` follows two rules: renew before spending a request whenever
the stored token is at or past its own expiry, the ordinary case rather
than an edge case, since Claude Code's tokens live roughly 8 hours; and
on an unexpected 401, renew once, retrying only if the credential
actually changed, per "Sign-in recovery" below.

`security(1)`'s exit status for "no such Keychain item",
`errSecItemNotFound`, is not documented anywhere Apple publishes; `44` is
a measured value, not a citable constant, and is the only exit code
`classify_keychain_failure` treats as "not signed in" rather than a
local problem such as a denied ACL or a locked keychain. Confirm it
against a real `security find-generic-password` run before ever
changing it.

A 200 whose body cannot be read, whether from a mid-body reset, a
timeout, or a proxy's HTML error page, is a failed read, full stop:
`get_json` returns `FetchError::Network` rather than swallowing it as
`Null`, which would be indistinguishable from a genuine "no limits to
report" answer, a healthy-looking read that silently wipes every
window. A non-200 answer is classified by its status alone, and its body
is never consumed, so an unreadable one changes nothing.

`fetch_profile` is fetched at most once per account per app run and then
cached by the caller, since the account name and plan it reports almost
never change within a single run.

## `CLAUDE_CONFIG_DIR`

Setting `CLAUDE_CONFIG_DIR` to the default account's own directory is not
the same as leaving it unset. With the variable set, the Claude Code CLI
reads a different on-disk config path and derives a different, hashed
Keychain service name for it, one that does not exist for the default
account. `claude_config_dir_env` in `providers/claude.rs` is the single
place this is decided: it returns `None`, meaning the variable must not
be set, whenever the configured directory is the default one.

## Finding the `claude` CLI

A menu bar app launched from Finder or the Dock inherits no `PATH`, so a
bare `Command::new("claude")` only ever finds `/usr/bin:/bin:/usr/sbin:/sbin`
and never a per-user install. `claude_cli_path` layers `$PATH`, then a
login-shell probe of the running system, then Claude Code's own
documented install locations rebuilt from `$HOME`, never hardcoded for
one Mac.

`ask_login_shell` tries both `-lc` and `-ilc`, in that order: a login
shell alone is not enough when `PATH` is set in a file such as
`.zshrc`, which only an interactive shell reads, so a plain `-lc` login
shell can find nothing while `-ilc` resolves the same command
correctly. The cheaper, quieter `-lc` form goes first; the interactive
form is the fallback, bounded by the same deadline in case an rc file
misbehaves.

## Sign-in recovery

`signin.rs` drives Claude Code's own login rather than anything owned by
Quotos: it spawns `claude setup-token` attached to a real pty, since the
CLI renders an interactive, cursor-positioning prompt using ANSI cursor
movement rather than plain line output, and a plain pipe risks the CLI
detecting a non-tty stdin and refusing or silently changing that
prompt's behavior. The CLI opens the browser and prints its own
authorization URL; Quotos never parses that output, never opens a
browser, and never reads or writes a credential itself. It only starts
the process pointed at the right account's `CLAUDE_CONFIG_DIR`, relays a
pasted code from the panel's own field into the process's stdin, and
notices when it is done; it never reads or displays the pty's own
output. Completion is detected by the process exiting, at which point
Quotos re-reads the account normally; the re-read, not the exit status,
is what actually proves the sign-in worked.

`FetchError::Unauthorized` means the sign-in itself has ended.
`FetchError::CredentialStale` means the access token merely aged out and
is still renewable, a local problem, not an expired sign-in. The 401
path only retries after re-reading the credential and confirming it
actually changed, so a renewal that does nothing never costs a second
request or gets reported as an expired sign-in. An expiring token is
renewed before a request is spent on it, so an ordinary access-token
expiry never reaches the user as a 401 at all.

`SignInRegistry::start` drops its own copy of `pair.slave` right after
spawning the child: the pty only signals end-of-stream once every slave
handle is closed, so holding onto this copy would leave the output-drain
thread's read loop blocked forever after the child actually exits. That
drain thread exists because an unread pty master's buffer fills and
blocks the child's own writes; `claude setup-token`'s output is never
inspected, but it still has to be pulled continuously. `master` must
outlive `child` for the same reason in reverse: dropping it while the
child is still running can tear down the pty out from under it, so the
wait-and-notify thread takes ownership of both and holds `master` until
`child.wait()` returns. `cancel` uses its own `killer` handle, cloned
before that thread was spawned, so cancelling a session never contends
with the thread that is blocked waiting on it.

`run_cli_credential_refresh` renews a stale access token without
spending an HTTP request of Quotos's own: any CLI invocation refreshes
and writes back the stored credential, and `claude mcp list` does no
inference beyond that, since an expired token's `expiresAt` advances
across exactly this call. That it ran is not proof anything was
renewed; only re-reading the credential afterward can distinguish an
actual renewal from the CLI running without changing anything.

## The statusline feed

While an interactive Claude Code session is running, its statusline hook
reports the same `rate_limits` numbers the usage endpoint does, without
Quotos spending a request of its own to get them. `statusline.rs`
generates a small POSIX `sh` script per enabled account, opt-in from the
Subscriptions screen, at
`<app config dir>/claude-statusline/<slug>.sh` (`slug_for` hashes the
account's own config dir). The script is self-contained: the feed path
and, if the account already had a `statusLine`, that previous command
are baked in as shell-quoted literals at generation time, so the script
reads stdin, no arguments, no `jq`, and no Quotos binary involved at
all.

The script does exactly three things: read stdin into a variable; write
it verbatim, atomically (a temp file in the same directory, then `mv
-f`), to the feed file `<slug>.json` next to it; then either pipe that
same payload into the wrapped previous command and pass its stdout
through unchanged, or, if there was no previous status line, print the
single line `Quotos is listening`. Claude Code's own docs state that a
nonzero exit or malformed `statusLine` output just blanks that row, so
the script never tries to fail loudly; a failed write to the feed file
still lets the emit step run.

Writing `settings.json` follows the same read-merge-write discipline as
before: refusing and changing nothing on a parse failure, preserving
every other key, writing atomically through a temp file and rename in
the same directory, and preserving `padding` and `refreshInterval` from
a wrapped entry. There is no conflict state to refuse into any more:
enabling always wraps whatever `statusLine` command was already there.
Enabled means exactly one thing, checked fresh every time —
`settings.json`'s `statusLine.command` equals the command Quotos would
generate for that account — so a status line the user changed by hand
through `/statusline` simply reads as off, nothing more to reconcile.
Disabling restores the previous `statusLine` value exactly (or clears
the key when there was none), deletes the script, the feed file, and
that account's single timestamped `settings.json` backup, and removes
the `claude-statusline` directory once it's empty.

Anyone who enabled the old copied-binary version is migrated once, on
app start: `migrate_legacy` finds any of the three legacy directories it
left behind, and for every old backup record whose account's
`settings.json` still points at the old copied binary, regenerates the
new script wrapping that record's previous status line and rewrites the
entry; a record whose
settings no longer point at us is left untouched. The three legacy
directories are then removed unconditionally, so a second run is a
silent no-op.

This is a scoped exception to the provider-adapter seam in
[architecture.md](architecture.md): the feed's own vocabulary,
`five_hour` and `seven_day`, is Claude Code CLI vocabulary, not a
generic shape, but the generate, backup, restore, and read plumbing is
per-config-dir infrastructure with nothing Claude-specific in how it
works. This puts it in the same category as `persistence.rs` and
`scheduler.rs`, which are shell-owned even though Claude is their only
current caller.

Reading a feed file never trusts a `written_at` field the script might
have written — there isn't one, since the script has no way to produce
a timestamp portably. `read_feed_file` takes the file's own mtime as the
observation time instead, which is also what `spawn_statusline_watcher`
relies on: it watches `claude-statusline` with the `notify` crate
(FSEvents on macOS), and on a change to a feed file, re-emits
`quota-refresh` for the matching tracked account by pairing the fresh
feed with that account's last real API read from `last_ok_snapshot`, so
the tray and panel update within about a second of each Claude Code turn
without spending a request. The periodic scheduler tick still reads the
feed file directly on every pass, so a missed FSEvent, or a feed update
that arrives before any API read has ever populated the cache, costs at
most one tick.

On the frontend, `statusline-merge.ts`'s `reconcileWithStatusline` patches
the raw usage shape before `normalizeUsage` ever sees it, so headline
selection and severity apply exactly as they would to a fresher API
response. Freshest wins by comparing the feed's own timestamp against the
API read's `fetched_at`, which is stamped when the HTTP response
arrives, not when a snapshot is assembled afterward. The feed only ever
refreshes a window the API response already asserts exists, and a
fresher reading that is missing a reset time keeps the API's own reset
time rather than blanking it.
