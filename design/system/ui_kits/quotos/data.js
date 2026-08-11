/* Quotos UI kit — mock data & helpers (plain globals, no build step). */
(function () {
  // A realistic mixed-state set: working (incl. one critical), behind, waiting,
  // broken, not-connected. Two are pinned to the menu bar.
  window.QUOTOS_SEED = [
    {
      id: "max", label: "Claude Max", provider: "Anthropic", account: "Personal",
      state: "working", remaining: 62, resetLabel: "resets in 3h", lastRead: "2 min ago",
      pinned: true,
      windows: [
        { name: "Session", remaining: 62, resetLabel: "resets in 3h" },
        { name: "Weekly", remaining: 41, resetLabel: "resets Mon", scope: "Opus 4" },
        { name: "Code review", resetLabel: null },
      ],
    },
    {
      id: "repair", label: "Claude Max", provider: "Anthropic", account: "Personal · 2",
      state: "repairing", remaining: 71, resetLabel: "resets in 6h", lastRead: "5 min ago",
      pinned: false,
      windows: [
        { name: "Session", remaining: 71, resetLabel: "resets in 6h" },
      ],
    },
    {
      id: "team", label: "Claude Team", provider: "Anthropic", account: "Work",
      state: "working", remaining: 8, resetLabel: "resets in 90 min", lastRead: "2 min ago",
      pinned: true,
      windows: [
        { name: "Session", remaining: 8, resetLabel: "resets in 90 min" },
        { name: "Weekly", remaining: 34, resetLabel: "resets Thu" },
      ],
    },
    {
      id: "eu", label: "Claude Team (EU)", provider: "Anthropic", account: "Work · Frankfurt",
      state: "behind", remaining: 40, resetLabel: "resets in 5h", lastRead: "41 min ago",
      pinned: false,
      windows: [
        { name: "Session", remaining: 40, resetLabel: "resets in 5h" },
        { name: "Wöchentliches Limit", remaining: 71, resetLabel: null },
      ],
    },
    {
      id: "api", label: "Research key", provider: "Anthropic", account: "API",
      state: "waiting", remaining: 88, resetLabel: "resets weekly", lastRead: "just now",
      pinned: false,
      windows: [
        { name: "Weekly", remaining: 88, resetLabel: "resets Sun" },
        { name: "Opus", remaining: 55, resetLabel: null, scope: "Opus 4" },
        { name: "Sonnet", remaining: 92, resetLabel: null, scope: "Sonnet" },
      ],
    },
    {
      id: "flex", label: "Flex tier", provider: "Anthropic", account: "API",
      state: "working", remaining: null, resetLabel: null, lastRead: "just now",
      pinned: false, windows: [],
    },
    {
      id: "old", label: "Old account", provider: "Anthropic", account: "Personal",
      state: "broken", remaining: null, resetLabel: null, lastRead: "1h ago",
      reason: "Sign-in expired. Reconnect to resume reading.",
      actionLabel: "Reconnect", pinned: false, windows: [],
    },
    {
      id: "new", label: "Team seat", provider: "Anthropic", account: "Not connected",
      state: "idle", remaining: null, resetLabel: null, lastRead: null,
      actionLabel: "Finish setup", pinned: false, windows: [],
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
      remaining: 74, resetLabel: "resets in 4h",
      windows: [
        { name: "Session", remaining: 74, resetLabel: "resets in 4h" },
        { name: "Weekly", remaining: 58, resetLabel: "resets Mon" },
      ],
    },
  };

  // Nudge a percentage a little, clamped — used to make refresh feel live.
  window.quotosJitter = function (n) {
    if (typeof n !== "number") return n;
    return Math.max(1, Math.min(99, n - Math.floor(Math.random() * 3)));
  };
})();
