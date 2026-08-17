# Security policy

Quotos runs entirely on your Mac. It reads Claude Code's own
Keychain-backed OAuth credential to call Claude's usage and profile
endpoints directly, keeps its own tracked-subscription list in a local
JSON file, and, only when you opt in per subscription, edits Claude
Code's `settings.json` to install a statusline helper. There is no
Quotos server and no Quotos account: everything it reads or writes
stays on the machine it runs on. See
[docs/claude-provider.md](docs/claude-provider.md) for the exact
surface.

## Reporting a vulnerability

Please report a vulnerability privately, through this repository's
Security tab and its "Report a vulnerability" button, rather than
opening a public issue.

Quotos is maintained by one person, so allow a few days for a first
response.

## Scope

Reports we especially care about:

- Anything that reads, writes, or exports a Keychain credential beyond
  the one OAuth token Quotos is meant to read.
- A change to Claude Code's own `settings.json`, or its statusline
  helper, that runs outside the opt-in described in
  [docs/claude-provider.md](docs/claude-provider.md).
- Anything that sends account data anywhere other than Claude's own
  API, since Quotos has no server of its own to send it to.

Out of scope: issues that need an attacker who already controls your
own Mac account, since they already have the Keychain entry directly.

## Supported versions

Only the latest commit on `main` is supported. There are no maintained
release lines.
