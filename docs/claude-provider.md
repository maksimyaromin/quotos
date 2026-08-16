# The Claude provider

Everything here is specific to the Claude Code adapter,
`src-tauri/src/providers/claude.rs` and `src/providers/claude/`. See
[architecture.md](architecture.md) for the generic provider seam this
adapter plugs into.

## Reading the account

Quotos reads a Keychain-backed OAuth token and calls the undocumented but
first-party `/api/oauth/usage` and `/api/oauth/profile` endpoints. That
endpoint is rate-limited at 5 requests per 300 seconds per account,
shared with Claude Code itself; `ratelimit.rs` is the budget that keeps
Quotos inside it.

The Keychain item's own decrypt and export ACL trusts exactly
`/usr/bin/security` with `promptSelector=0`, so the `security
find-generic-password -w` call this app shells out to is authorized and
never raises a sign-in prompt. `QUOTOS_DEBUG_READS=1` traces the
credential and read decisions, never a token, to standard error.

`fetch_usage` follows three rules: renew before spending a request
whenever the stored token is at or past its own expiry, the ordinary
case rather than an edge case, since Claude Code's tokens live roughly 8
hours; on an unexpected 401, renew once, retrying only if the credential
actually changed, per "Sign-in recovery" below; and reserve one budget
slot per real request, never per read attempt, per "Refresh scheduling
and the shared request budget" in architecture.md.

A 200 whose body cannot be read, whether from a mid-body reset, a
timeout, or a proxy's HTML error page, is a failed read, full stop:
`get_json` returns `FetchError::Network` rather than swallowing it as
`Null`, which would be indistinguishable from a genuine "no limits to
report" answer, a healthy-looking read that silently wipes every
window. A non-200 answer is classified by its status alone, and its body
is never consumed, so an unreadable one changes nothing.

`fetch_profile` is deliberately outside the request budget and fetched
at most once per account per app run, since the caller caches it: the
fixed one-read-per-minute cadence already consumes the whole 5-per-300s
allowance, so charging this call too would make the limiter refuse a
scheduled read every launch. One un-budgeted request per account per run
is a bounded, documented overshoot.

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

`run_cli_credential_refresh` renews a stale access token at zero cost
against the shared request budget: any CLI invocation refreshes and
writes back the stored credential, and `claude mcp list` does no
inference beyond that, since an expired token's `expiresAt` advances
across exactly this call. That it ran is not proof anything was
renewed; only re-reading the credential afterward can distinguish an
actual renewal from the CLI running without changing anything.

## The statusline feed

While an interactive Claude Code session is running, its statusline hook
reports the same `rate_limits` numbers the usage endpoint does, at no
cost against the shared request budget. `statusline.rs` installs a tiny
helper as that hook, opt-in per subscription from the Subscriptions
screen, by read-merge-writing the Claude Code config's `settings.json`
and backing up whatever `statusLine` value was there before.

The installed helper is a copy of Quotos's own executable, not a
separate binary: `main.rs` intercepts an ingest flag as `argv[1]` before
touching Tauri at all, so the copy Claude Code's hook invokes, possibly
many times a minute, never starts a second GUI instance. Reusing the
whole GUI binary avoids Tauri's `externalBin` sidecar bundling,
target-triple-suffixed binaries staged into a `binaries/` folder before
`tauri build`, for a purpose-built helper that would only ever need to
spawn, parse one JSON payload, and exit; the tradeoff is tens of extra
megabytes on disk, not correctness, for a local desktop app.

Writing `settings.json` follows six rules: only on an explicit in-app
opt-in per subscription, never automatic; read-merge-write, refusing and
changing nothing on a parse failure, preserving every other key, and
writing atomically through a temp file and rename in the same directory;
never clobbering a `statusLine` that is already configured and
different, unless the caller forces a replace; a timestamped backup of
the previous file plus a small metadata record of the previous
`statusLine` value, which removal restores exactly; the installed
command points at the copied helper described above; and the feed is
always a second source; the frontend's reconciliation, not the Rust
side, decides which reading wins. `statusline.rs` never invents a window
the API did not already report.

This is a scoped exception to the provider-adapter seam in
[architecture.md](architecture.md): the feed's own vocabulary,
`five_hour` and `seven_day`, is Claude Code CLI vocabulary, not a
generic shape, but the install, backup, restore, and read plumbing is
per-config-dir infrastructure with nothing Claude-specific in how it
works. This puts it in the same category as `persistence.rs` and
`scheduler.rs`, which are shell-owned even though Claude is their only
current caller.

On the frontend, `statusline-merge.ts`'s `reconcileWithStatusline` patches
the raw usage shape before `normalizeUsage` ever sees it, so headline
selection and severity apply exactly as they would to a fresher API
response. Freshest wins by comparing the feed's own timestamp against the
API read's `fetched_at`, which is stamped when the HTTP response
arrives, not when a snapshot is assembled afterward. The feed only ever
refreshes a window the API response already asserts exists, and a
fresher reading that is missing a reset time keeps the API's own reset
time rather than blanking it.
