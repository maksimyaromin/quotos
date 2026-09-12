---
id: decision.no-local-request-budget
type: decision
state: active
kind: rule
title: Only the provider's own 429 holds a read back
by: Maksim Yaromin
created: 2026-09-12
updated: 2026-09-12
---

Quotos does not model a provider's rate limit itself. No request is
ever refused by Quotos's own count of requests made. The only thing
that ever holds a read back is the provider's own 429 and the
`Retry-After` header that comes with it.

A local request budget existed once and was removed. It cannot be
right. It has no view of requests the same credentials made from
anywhere else; its ceiling is a guess at a number the provider is free
to change; and when the guess is low it refuses reads the provider
would have served, which reads to the user as the app being broken
rather than as the app being careful. When the guess is high it
prevents nothing, because the provider's 429 arrives anyway and is the
thing actually handled.

What honours a wait is narrow on purpose. Only the native scheduler's
automatic cadence skips a due read while a `Retry-After` wait stands,
floored at one minute, since an automatic retry sooner would draw a
second 429 and nothing else. A manual refresh, and opening the panel,
always reach the provider, so a wrong diagnosis is never impossible to
retest. A pending wait is also kept apart from a subscription's health,
so a 429 never overwrites a real diagnosis.

Anything that would count requests, precompute a ceiling, or refuse a
read before sending it belongs on the provider's side of this line, not
in Quotos. docs/architecture.md's refresh scheduling section describes
the cadence that remains.
