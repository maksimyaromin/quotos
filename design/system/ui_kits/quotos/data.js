/* Quotos UI kit — mock data & helpers (plain globals, no build step).
   Percent fields are consumed ("used"), never remaining; states are the
   shipped six (working / reading / behind / broken / connecting / idle);
   pinning is per-window. */
(function () {
  // A realistic mixed-state set: working (one with a critical window),
  // reading, behind, connecting, no-limits, broken, not-connected.
  // Two windows are pinned to the menu bar.
  window.QUOTOS_SEED = [
    {
      id: "max", label: "Claude Max", provider: "Anthropic", account: "Personal",
      state: "working", used: 38, severity: "warn", resetLabel: "Resets today at 4:05 PM", lastRead: "2 min ago",
      pinnedCount: 1, headlinePinned: true,
      windows: [
        { id: "max-session", name: "Session", used: 38, resetLabel: "Resets today at 4:05 PM", pinned: true },
        { id: "max-weekly", name: "Weekly", used: 82, resetLabel: "Resets Mon", scope: "Opus 4" },
        { id: "max-review", name: "Code review", resetLabel: null },
      ],
    },
    {
      id: "second", label: "Claude Max", provider: "Anthropic", account: "Personal · 2",
      state: "reading", used: 29, severity: "healthy", resetLabel: "Resets in 6h", lastRead: "5 min ago",
      pinnedCount: 0, headlinePinned: false,
      windows: [
        { id: "second-session", name: "Session", used: 29, resetLabel: "Resets in 6h" },
      ],
    },
    {
      id: "team", label: "Claude Team", provider: "Anthropic", account: "Work",
      state: "working", used: 92, severity: "critical", resetLabel: "Resets in 90 min", lastRead: "2 min ago",
      pinnedCount: 1, headlinePinned: true,
      windows: [
        { id: "team-session", name: "Session", used: 92, resetLabel: "Resets in 90 min", pinned: true },
        { id: "team-weekly", name: "Weekly", used: 66, resetLabel: "Resets Thu" },
      ],
    },
    {
      id: "eu", label: "Claude Team (EU)", provider: "Anthropic", account: "Work · Frankfurt",
      state: "behind", used: 60, severity: "healthy", resetLabel: "Resets in 5h", lastRead: "41 min ago",
      badge: "Not current", actionLabel: "Try again",
      pinnedCount: 0, headlinePinned: false,
      windows: [
        { id: "eu-session", name: "Session", used: 60, resetLabel: "Resets in 5h" },
        { id: "eu-weekly", name: "Wöchentliches Limit", used: 29, resetLabel: null },
      ],
    },
    {
      id: "api", label: "Research key", provider: "Anthropic", account: "API",
      state: "working", used: 12, severity: "healthy", resetLabel: "Resets Sun", lastRead: "just now",
      footerNote: "Waiting for the rate budget · 40 s",
      pinnedCount: 0, headlinePinned: false,
      windows: [
        { id: "api-weekly", name: "Weekly", used: 12, resetLabel: "Resets Sun" },
        { id: "api-opus", name: "Opus", used: 45, resetLabel: null, scope: "Opus 4" },
        { id: "api-sonnet", name: "Sonnet", used: 8, resetLabel: null, scope: "Sonnet" },
      ],
    },
    {
      id: "flex", label: "Flex tier", provider: "Anthropic", account: "API",
      state: "working", used: null, severity: "healthy", resetLabel: null, lastRead: "just now",
      pinnedCount: 0, headlinePinned: false, windows: [],
    },
    {
      id: "old", label: "Old account", provider: "Anthropic", account: "Personal",
      state: "broken", used: null, resetLabel: null, lastRead: "1h ago",
      reason: "Sign-in expired. Sign in to resume reading.",
      badge: "Needs sign-in", actionLabel: "Sign in",
      pinnedCount: 0, headlinePinned: false, windows: [],
    },
    {
      id: "new", label: "Team seat", provider: "Anthropic", account: "Not connected",
      state: "idle", used: null, resetLabel: null, lastRead: null,
      actionLabel: "Finish setup",
      pinnedCount: 0, headlinePinned: false, windows: [],
    },
  ];

  // The add-subscription flow content.
  window.QUOTOS_ADD = {
    found: [
      { id: "f1", label: "claude.ai", detail: "Signed in as you@studio.dev · Max" },
      { id: "f2", label: "Claude Code CLI", detail: "Credentials found in keychain" },
    ],
    providers: [
      { id: "anthropic", label: "Anthropic", detail: "Claude — Max, Team, API", available: true },
      { id: "more", label: "More providers", detail: "Coming after the first version", available: false },
    ],
    methods: [
      { id: "cli", label: "Use Claude Code credentials", detail: "Reuse the key the CLI already stores. No pasting.", recommended: true },
      { id: "session", label: "Use claude.ai session", detail: "Read using your logged-in browser session." },
      { id: "key", label: "Paste an API key", detail: "For an API subscription. Starts with sk-ant-." },
    ],
    // A sample verified reading shown on the result step.
    verified: {
      label: "Claude Max", account: "Personal", provider: "Anthropic",
      used: 26, resetLabel: "resets in 4h",
      windows: [
        { id: "v-session", name: "Session", used: 26, resetLabel: "Resets in 4h" },
        { id: "v-weekly", name: "Weekly", used: 42, resetLabel: "Resets Mon" },
      ],
    },
  };

  // Nudge a consumed percentage up a little, clamped — makes refresh feel live.
  window.quotosJitter = function (n) {
    if (typeof n !== "number") return n;
    return Math.max(1, Math.min(99, n + Math.floor(Math.random() * 3)));
  };
})();
