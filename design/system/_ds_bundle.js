/* @ds-bundle: {"format":4,"namespace":"QuotosDesignSystem_52e720","components":[{"name":"Button","sourcePath":"components/controls/Button.jsx"},{"name":"IconButton","sourcePath":"components/controls/IconButton.jsx"},{"name":"TextField","sourcePath":"components/controls/TextField.jsx"},{"name":"Badge","sourcePath":"components/indicators/Badge.jsx"},{"name":"CapacityBar","sourcePath":"components/indicators/CapacityBar.jsx"},{"name":"StatusDot","sourcePath":"components/indicators/StatusDot.jsx"},{"name":"QuotaGlyph","sourcePath":"components/shell/MenuBarTile.jsx"},{"name":"MenuBarTile","sourcePath":"components/shell/MenuBarTile.jsx"},{"name":"Panel","sourcePath":"components/shell/Panel.jsx"},{"name":"LimitWindow","sourcePath":"components/subscription/LimitWindow.jsx"},{"name":"SubscriptionRow","sourcePath":"components/subscription/SubscriptionRow.jsx"}],"sourceHashes":{"components/controls/Button.jsx":"15f1424afbe1","components/controls/IconButton.jsx":"2907f265ea8e","components/controls/TextField.jsx":"22e2995ba284","components/indicators/Badge.jsx":"1e403801f18a","components/indicators/CapacityBar.jsx":"64bb7a20782c","components/indicators/StatusDot.jsx":"b334806a14b2","components/shell/MenuBarTile.jsx":"a39354bf0c87","components/shell/Panel.jsx":"0f8baecfacc2","components/subscription/LimitWindow.jsx":"2c3aab092844","components/subscription/SubscriptionRow.jsx":"2101c0c15629","ui_kits/quotos/data.js":"782977888514"},"inlinedExternals":[],"unexposedExports":[{"name":"capacityColor","sourcePath":"components/indicators/CapacityBar.jsx"}]} */

(() => {

const __ds_ns = (window.QuotosDesignSystem_52e720 = window.QuotosDesignSystem_52e720 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/controls/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const base = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "var(--space-1-5)",
  fontFamily: "var(--font-sans)",
  fontWeight: "var(--weight-medium)",
  lineHeight: 1,
  border: "0.5px solid transparent",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  userSelect: "none",
  whiteSpace: "nowrap",
  transition: "background var(--dur-fast) var(--ease-standard), border-color var(--dur-fast) var(--ease-standard), opacity var(--dur-fast), transform var(--dur-instant)"
};
const sizes = {
  sm: {
    height: "22px",
    padding: "0 var(--space-2)",
    fontSize: "var(--text-sm)"
  },
  base: {
    height: "var(--control-height)",
    padding: "0 var(--space-3)",
    fontSize: "var(--text-base)"
  },
  lg: {
    height: "var(--control-height-lg)",
    padding: "0 var(--space-4)",
    fontSize: "var(--text-md)"
  }
};
const variants = {
  primary: {
    background: "var(--teal)",
    color: "var(--text-on-accent)",
    borderColor: "color-mix(in oklch, var(--teal) 70%, black)"
  },
  secondary: {
    background: "var(--bg-input)",
    color: "var(--text-primary)",
    borderColor: "var(--border-default)"
  },
  ghost: {
    background: "transparent",
    color: "var(--text-secondary)",
    borderColor: "transparent"
  },
  danger: {
    background: "transparent",
    color: "var(--red)",
    borderColor: "var(--border-default)"
  }
};

/** A labeled action button. Named, sentence-case labels; verbs. */
function Button({
  variant = "secondary",
  size = "base",
  disabled = false,
  fullWidth = false,
  icon = null,
  onClick,
  children,
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const [active, setActive] = React.useState(false);
  const hoverBg = {
    primary: "var(--teal-bright)",
    secondary: "var(--bg-row-hover)",
    ghost: "var(--bg-row-hover)",
    danger: "var(--red-muted)"
  }[variant];
  const composed = {
    ...base,
    ...sizes[size],
    ...variants[variant],
    ...(hover && !disabled ? {
      background: hoverBg
    } : null),
    ...(active && !disabled ? {
      transform: "translateY(0.5px)",
      opacity: 0.9
    } : null),
    ...(disabled ? {
      opacity: 0.4,
      cursor: "default"
    } : null),
    ...(fullWidth ? {
      width: "100%"
    } : null),
    ...style
  };
  return /*#__PURE__*/React.createElement("button", _extends({
    type: "button",
    disabled: disabled,
    onClick: disabled ? undefined : onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => {
      setHover(false);
      setActive(false);
    },
    onMouseDown: () => setActive(true),
    onMouseUp: () => setActive(false),
    style: composed
  }, rest), icon ? /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      width: 14,
      height: 14
    }
  }, icon) : null, children);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/Button.jsx", error: String((e && e.message) || e) }); }

// components/controls/IconButton.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** A square, borderless icon control for toolbar-style actions (refresh, close,
 *  settings, expand). Monochrome; inherits currentColor. */
function IconButton({
  size = 24,
  active = false,
  disabled = false,
  label,
  onClick,
  children,
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const composed = {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: size,
    height: size,
    padding: 0,
    border: 0,
    borderRadius: "var(--radius-sm)",
    background: hover && !disabled ? "var(--bg-row-hover)" : "transparent",
    color: active ? "var(--text-accent)" : "var(--text-secondary)",
    cursor: disabled ? "default" : "pointer",
    opacity: disabled ? 0.35 : 1,
    transition: "background var(--dur-fast) var(--ease-standard), color var(--dur-fast)",
    ...style
  };
  return /*#__PURE__*/React.createElement("button", _extends({
    type: "button",
    "aria-label": label,
    title: label,
    disabled: disabled,
    onClick: disabled ? undefined : onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: composed
  }, rest), /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      width: Math.round(size * 0.6),
      height: Math.round(size * 0.6)
    }
  }, children));
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/controls/TextField.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** A labeled text input in macOS small-control style. Used in the
 *  add-subscription flow (paste a key, name an account). */
function TextField({
  label,
  value,
  placeholder,
  mono = false,
  invalid = false,
  disabled = false,
  onChange,
  style,
  ...rest
}) {
  const [focus, setFocus] = React.useState(false);
  return /*#__PURE__*/React.createElement("label", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-1)",
      ...style
    }
  }, label ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: "var(--text-secondary)"
    }
  }, label) : null, /*#__PURE__*/React.createElement("input", _extends({
    value: value,
    placeholder: placeholder,
    disabled: disabled,
    onChange: onChange,
    onFocus: () => setFocus(true),
    onBlur: () => setFocus(false),
    style: {
      height: "var(--control-height-lg)",
      padding: "0 var(--space-2)",
      fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
      fontSize: "var(--text-base)",
      color: "var(--text-primary)",
      background: "var(--bg-input)",
      border: `0.5px solid ${invalid ? "var(--red)" : focus ? "var(--border-focus)" : "var(--border-default)"}`,
      borderRadius: "var(--radius-sm)",
      outline: "none",
      boxShadow: focus ? "var(--shadow-focus)" : "none",
      transition: "border-color var(--dur-fast), box-shadow var(--dur-fast)"
    }
  }, rest)));
}
Object.assign(__ds_scope, { TextField });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/TextField.jsx", error: String((e && e.message) || e) }); }

// components/indicators/Badge.jsx
try { (() => {
const TONES = {
  neutral: {
    color: "var(--text-tertiary)",
    bg: "var(--bg-elevated)",
    border: "var(--border-default)"
  },
  accent: {
    color: "var(--text-accent)",
    bg: "var(--teal-muted)",
    border: "transparent"
  },
  warn: {
    color: "var(--amber)",
    bg: "var(--amber-muted)",
    border: "transparent"
  },
  danger: {
    color: "var(--red)",
    bg: "var(--red-muted)",
    border: "transparent"
  },
  info: {
    color: "var(--blue)",
    bg: "var(--blue-muted)",
    border: "transparent"
  }
};

/** A small pill for a short status word or a window scope tag. Sentence-case,
 *  never louder than it needs to be. */
function Badge({
  tone = "neutral",
  children,
  style
}) {
  const t = TONES[tone] || TONES.neutral;
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      height: "16px",
      padding: "0 var(--space-1-5)",
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      fontWeight: "var(--weight-medium)",
      lineHeight: 1,
      color: t.color,
      background: t.bg,
      border: `0.5px solid ${t.border}`,
      borderRadius: "var(--radius-xs)",
      whiteSpace: "nowrap",
      ...style
    }
  }, children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/indicators/Badge.jsx", error: String((e && e.message) || e) }); }

// components/indicators/CapacityBar.jsx
try { (() => {
/** Maps remaining capacity to a fill color: teal when healthy, amber when
 *  getting low, red when nearly out. This is the only place color changes
 *  meaning by value. */
function capacityColor(remaining) {
  if (remaining <= 10) return "var(--cap-critical)";
  if (remaining <= 25) return "var(--cap-warn)";
  return "var(--cap-healthy)";
}

/** The thin depleting bar. `remaining` (0–100) sets fill width and color.
 *  When `reading`, an indeterminate shimmer plays over the held value —
 *  the number is never blanked. When `stale`, the fill dims. */
function CapacityBar({
  remaining = 0,
  reading = false,
  stale = false,
  height,
  style
}) {
  const fill = Math.max(0, Math.min(100, remaining));
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "relative",
      width: "100%",
      height: height || "var(--cap-bar-height)",
      background: "var(--cap-track)",
      borderRadius: "var(--radius-pill)",
      overflow: "hidden",
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      position: "absolute",
      inset: 0,
      width: `${fill}%`,
      background: capacityColor(remaining),
      borderRadius: "var(--radius-pill)",
      opacity: stale ? 0.4 : 1,
      transition: "width var(--dur-slow) var(--ease-out), background var(--dur-base), opacity var(--dur-base)"
    }
  }), reading ? /*#__PURE__*/React.createElement("div", {
    style: {
      position: "absolute",
      inset: 0,
      background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.28), transparent)",
      backgroundSize: "40% 100%",
      backgroundRepeat: "no-repeat",
      animation: "quotos-shimmer 1.1s var(--ease-standard) infinite"
    }
  }) : null, /*#__PURE__*/React.createElement("style", null, `@keyframes quotos-shimmer{0%{background-position:-40% 0}100%{background-position:140% 0}}`));
}
Object.assign(__ds_scope, { capacityColor, CapacityBar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/indicators/CapacityBar.jsx", error: String((e && e.message) || e) }); }

// components/indicators/StatusDot.jsx
try { (() => {
const STATE_COLOR = {
  idle: "var(--status-idle)",
  connecting: "var(--status-progress)",
  working: "var(--status-working)",
  reading: "var(--status-progress)",
  behind: "var(--status-behind)",
  repairing: "var(--status-progress)",
  broken: "var(--status-broken)",
  waiting: "var(--status-waiting)"
};
const PULSING = new Set(["connecting", "reading", "repairing"]);

/** A small state dot. Color maps to subscription state; in-progress states
 *  pulse gently. "working" reads as calm (teal), not a loud green "OK". */
function StatusDot({
  state = "working",
  size = 7,
  style
}) {
  const pulsing = PULSING.has(state);
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-block",
      width: size,
      height: size,
      borderRadius: "var(--radius-pill)",
      background: STATE_COLOR[state] || "var(--status-idle)",
      boxShadow: state === "idle" ? "inset 0 0 0 1.5px var(--status-idle)" : "none",
      backgroundClip: state === "idle" ? "content-box" : "border-box",
      opacity: state === "idle" ? 0.9 : 1,
      animation: pulsing ? "quotos-pulse 1.4s var(--ease-standard) infinite" : "none",
      ...style
    }
  }, /*#__PURE__*/React.createElement("style", null, `@keyframes quotos-pulse{0%,100%{opacity:1}50%{opacity:0.35}}`));
}
Object.assign(__ds_scope, { StatusDot });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/indicators/StatusDot.jsx", error: String((e && e.message) || e) }); }

// components/shell/MenuBarTile.jsx
try { (() => {
/** The placeholder menu-bar glyph — a quota ring. NOT a logo (Quotos has
 *  none); a functional macOS template mark, monochrome via currentColor. */
function QuotaGlyph({
  size = 15
}) {
  return /*#__PURE__*/React.createElement("svg", {
    width: size,
    height: size,
    viewBox: "0 0 16 16",
    fill: "none",
    style: {
      flex: "0 0 auto"
    }
  }, /*#__PURE__*/React.createElement("circle", {
    cx: "8",
    cy: "8",
    r: "5.4",
    stroke: "currentColor",
    strokeWidth: "1.4",
    opacity: "0.28"
  }), /*#__PURE__*/React.createElement("path", {
    d: "M4.46 12.02a5.4 5.4 0 1 1 7.08 0",
    stroke: "currentColor",
    strokeWidth: "1.9",
    strokeLinecap: "round"
  }));
}
const AttentionMark = () => /*#__PURE__*/React.createElement("svg", {
  width: "11",
  height: "11",
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: "2.2",
  strokeLinecap: "round",
  strokeLinejoin: "round"
}, /*#__PURE__*/React.createElement("path", {
  d: "M10.3 3.6 1.8 18a1.9 1.9 0 0 0 1.7 2.9h17a1.9 1.9 0 0 0 1.7-2.9L13.7 3.6a1.9 1.9 0 0 0-3.4 0Z"
}), /*#__PURE__*/React.createElement("path", {
  d: "M12 9v4M12 17h.01"
}));

/** The tray representation: the glyph plus optional pinned figures. Monochrome
 *  by macOS convention — a pinned figure only takes a warning tint (amber /
 *  red) when it actually needs attention; broken pins show a small mark instead
 *  of a stale number. Renders on a mock menu-bar strip for preview. */
function MenuBarTile({
  pins = [],
  onClick,
  showStrip = true,
  style
}) {
  const tintFor = p => {
    if (p.state === "broken" || p.state === "behind") return "var(--amber)";
    if (typeof p.remaining === "number" && p.remaining <= 10) return "var(--red)";
    if (typeof p.remaining === "number" && p.remaining <= 25) return "var(--amber)";
    return "inherit";
  };
  const tile = /*#__PURE__*/React.createElement("button", {
    type: "button",
    onClick: onClick,
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-1-5)",
      height: 22,
      padding: "0 var(--space-1-5)",
      border: 0,
      borderRadius: "var(--radius-xs)",
      background: "transparent",
      cursor: "pointer",
      color: showStrip ? "rgba(255,255,255,0.92)" : "var(--text-primary)",
      fontFamily: "var(--font-mono)",
      fontSize: "12px",
      fontWeight: "var(--weight-medium)",
      fontVariantNumeric: "tabular-nums"
    }
  }, /*#__PURE__*/React.createElement(QuotaGlyph, null), pins.map((p, i) => /*#__PURE__*/React.createElement("span", {
    key: i,
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: 3,
      color: tintFor(p)
    }
  }, p.state === "broken" ? /*#__PURE__*/React.createElement(AttentionMark, null) : /*#__PURE__*/React.createElement(React.Fragment, null, typeof p.remaining === "number" ? `${p.remaining}%` : "—", p.state === "behind" ? /*#__PURE__*/React.createElement("span", {
    style: {
      opacity: 0.7
    }
  }, /*#__PURE__*/React.createElement(AttentionMark, null)) : null))));
  if (!showStrip) return /*#__PURE__*/React.createElement("span", {
    style: style
  }, tile);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)",
      height: 24,
      padding: "0 var(--space-2)",
      background: "rgba(30,32,36,0.72)",
      backdropFilter: "saturate(160%) blur(20px)",
      WebkitBackdropFilter: "saturate(160%) blur(20px)",
      borderRadius: "var(--radius-sm)",
      ...style
    }
  }, tile, /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: 3,
      color: "rgba(255,255,255,0.6)",
      fontFamily: "var(--font-sans)",
      fontSize: 12
    }
  }, "100%", /*#__PURE__*/React.createElement("svg", {
    width: "22",
    height: "12",
    viewBox: "0 0 26 13",
    fill: "none"
  }, /*#__PURE__*/React.createElement("rect", {
    x: "0.5",
    y: "0.5",
    width: "22",
    height: "12",
    rx: "3",
    stroke: "currentColor",
    opacity: "0.7"
  }), /*#__PURE__*/React.createElement("rect", {
    x: "2",
    y: "2",
    width: "19",
    height: "9",
    rx: "1.5",
    fill: "currentColor"
  }), /*#__PURE__*/React.createElement("rect", {
    x: "23.5",
    y: "4",
    width: "2",
    height: "5",
    rx: "1",
    fill: "currentColor",
    opacity: "0.7"
  }))), /*#__PURE__*/React.createElement("span", {
    style: {
      color: "rgba(255,255,255,0.82)",
      fontFamily: "var(--font-sans)",
      fontSize: 12
    }
  }, "Mon 9:41"));
}
Object.assign(__ds_scope, { QuotaGlyph, MenuBarTile });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/MenuBarTile.jsx", error: String((e && e.message) || e) }); }

// components/shell/Panel.jsx
try { (() => {
/** The popover shell: a floating macOS vibrancy surface with a top beak, a
 *  header (title + toolbar actions), a scrollable body, and an optional footer.
 *  Depth is a single float — a soft shadow + hairline rim. */
function Panel({
  title = "Quotos",
  beak = true,
  headerActions = null,
  footer = null,
  maxBodyHeight = 460,
  children,
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "relative",
      width: "var(--panel-width)",
      ...style
    }
  }, beak ? /*#__PURE__*/React.createElement("div", {
    style: {
      position: "absolute",
      top: -6,
      left: "50%",
      transform: "translateX(-50%) rotate(45deg)",
      width: 12,
      height: 12,
      background: "var(--bg-panel)",
      borderTop: "0.5px solid var(--border-strong)",
      borderLeft: "0.5px solid var(--border-strong)",
      borderTopLeftRadius: 3,
      zIndex: 2
    }
  }) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      background: "var(--bg-panel)",
      backdropFilter: "var(--blur-vibrancy)",
      WebkitBackdropFilter: "var(--blur-vibrancy)",
      border: "0.5px solid var(--border-strong)",
      borderRadius: "var(--radius-xl)",
      boxShadow: "var(--shadow-popover)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)",
      padding: "var(--space-2-5) var(--space-3)",
      borderBottom: "0.5px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      flex: 1,
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-base)",
      fontWeight: "var(--weight-semibold)",
      color: "var(--text-primary)",
      letterSpacing: "var(--tracking-tight)"
    }
  }, title), headerActions), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-0-5)",
      padding: "var(--space-1-5)",
      maxHeight: maxBodyHeight,
      overflowY: "auto"
    }
  }, children), footer ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)",
      padding: "var(--space-2) var(--space-3)",
      borderTop: "0.5px solid var(--border-subtle)"
    }
  }, footer) : null));
}
Object.assign(__ds_scope, { Panel });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/Panel.jsx", error: String((e && e.message) || e) }); }

// components/subscription/LimitWindow.jsx
try { (() => {
/** One limit window inside a subscription's detail list. The list is variable:
 *  a window may lack a percentage or a reset time, and its name is the
 *  provider's own wording (rendered verbatim, possibly truncated). Renders
 *  gracefully whether there are 1 or 8 of these. */
function LimitWindow({
  name,
  remaining = null,
  resetLabel = null,
  scope = null,
  stale = false,
  style
}) {
  const hasPct = typeof remaining === "number";
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-1)",
      padding: "var(--space-1-5) 0",
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-1-5)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      flex: 1,
      minWidth: 0,
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: "var(--text-secondary)",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    }
  }, name), scope ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: "neutral"
  }, scope) : null, /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontSize: "var(--numeral-sm)",
      fontWeight: "var(--weight-medium)",
      fontVariantNumeric: "tabular-nums",
      color: hasPct ? "var(--text-primary)" : "var(--text-quaternary)",
      minWidth: 34,
      textAlign: "right"
    }
  }, hasPct ? `${remaining}%` : "—")), hasPct ? /*#__PURE__*/React.createElement(__ds_scope.CapacityBar, {
    remaining: remaining,
    stale: stale,
    height: "3px"
  }) : null, resetLabel ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      color: "var(--text-tertiary)"
    }
  }, resetLabel) : null);
}
Object.assign(__ds_scope, { LimitWindow });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/subscription/LimitWindow.jsx", error: String((e && e.message) || e) }); }

// components/subscription/SubscriptionRow.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const HAS_DATA = new Set(["working", "reading", "behind", "waiting", "repairing"]);

// Minimal default affordance glyphs (generic UI arrows/marks, not brand icons).
const Chevron = ({
  open
}) => /*#__PURE__*/React.createElement("svg", {
  width: "12",
  height: "12",
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: "2.2",
  strokeLinecap: "round",
  strokeLinejoin: "round",
  style: {
    transform: open ? "rotate(180deg)" : "none",
    transition: "transform var(--dur-base) var(--ease-standard)"
  }
}, /*#__PURE__*/React.createElement("path", {
  d: "M6 9l6 6 6-6"
}));
const PinGlyph = () => /*#__PURE__*/React.createElement("svg", {
  width: "13",
  height: "13",
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: "1.8",
  strokeLinecap: "round",
  strokeLinejoin: "round"
}, /*#__PURE__*/React.createElement("path", {
  d: "M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z"
}));
function TinyBtn({
  label,
  active,
  onClick,
  children
}) {
  const [h, setH] = React.useState(false);
  return /*#__PURE__*/React.createElement("button", {
    type: "button",
    "aria-label": label,
    title: label,
    onClick: onClick,
    onMouseEnter: () => setH(true),
    onMouseLeave: () => setH(false),
    style: {
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      gap: "var(--space-1)",
      height: 20,
      padding: "0 4px",
      border: 0,
      borderRadius: "var(--radius-xs)",
      cursor: "pointer",
      background: h ? "var(--bg-row-hover)" : "transparent",
      color: active ? "var(--text-accent)" : "var(--text-tertiary)",
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      transition: "background var(--dur-fast), color var(--dur-fast)"
    }
  }, children);
}
const STATE_META = {
  behind: {
    tone: "warn",
    label: "Behind"
  },
  waiting: {
    tone: "info",
    label: "Waiting on limits"
  },
  repairing: {
    tone: "info",
    label: "Repairing"
  },
  broken: {
    tone: "danger",
    label: "Broken"
  }
};

/** The core panel row: one subscription, scannable in a single pass. Renders a
 *  "% left" headline + capacity bar for data-bearing states, and a message +
 *  action for not-connected / connecting / broken. State-driven throughout;
 *  age is always shown, and stale data is visibly dimmed, never presented as
 *  current. Expands to the variable window list. */
function SubscriptionRow({
  label,
  provider,
  account,
  state = "working",
  remaining = null,
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  pinned = false,
  expanded = false,
  actionLabel = null,
  onAction,
  onTogglePin,
  onToggleExpand,
  style
}) {
  const [hover, setHover] = React.useState(false);
  const connected = HAS_DATA.has(state);
  const hasData = connected && typeof remaining === "number";
  const noLimits = connected && typeof remaining !== "number"; // read fine, provider reports nothing useful
  const stale = state === "behind";
  const reading = state === "reading";
  const meta = STATE_META[state];
  const numeralColor = stale ? "var(--text-tertiary)" : remaining !== null && remaining <= 25 ? __ds_scope.capacityColor(remaining) : "var(--text-primary)";
  return /*#__PURE__*/React.createElement("div", {
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-2)",
      padding: "var(--space-3)",
      borderRadius: "var(--radius-md)",
      background: hover ? "var(--bg-row-hover)" : "transparent",
      transition: "background var(--dur-fast) var(--ease-standard)",
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "flex-start",
      gap: "var(--space-2)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.StatusDot, {
    state: state,
    style: {
      marginTop: 4,
      flex: "0 0 auto"
    }
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-md)",
      fontWeight: "var(--weight-semibold)",
      color: "var(--text-primary)",
      letterSpacing: "var(--tracking-tight)",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    }
  }, label), provider || account ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: "var(--text-tertiary)",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    }
  }, [account, provider].filter(Boolean).join(" · ")) : null), meta ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: meta.tone,
    style: {
      marginTop: 1
    }
  }, meta.label) : null, /*#__PURE__*/React.createElement(TinyBtn, {
    label: pinned ? "Unpin from menu bar" : "Pin to menu bar",
    active: pinned,
    onClick: onTogglePin,
    style: {
      opacity: pinned || hover ? 1 : 0
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      opacity: pinned || hover ? 1 : 0,
      transition: "opacity var(--dur-fast)"
    }
  }, /*#__PURE__*/React.createElement(PinGlyph, null)))), hasData ? /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "flex-end",
      gap: "var(--space-2)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "baseline",
      gap: "var(--space-1)",
      flex: 1
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontSize: "var(--numeral-lg)",
      fontWeight: "var(--weight-medium)",
      fontVariantNumeric: "tabular-nums",
      lineHeight: 1,
      color: numeralColor,
      letterSpacing: "var(--tracking-tighter)"
    }
  }, remaining, /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: "18px"
    }
  }, "%")), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: "var(--text-tertiary)"
    }
  }, "left")), resetLabel ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      color: "var(--text-tertiary)",
      paddingBottom: 3
    }
  }, resetLabel) : null), /*#__PURE__*/React.createElement(__ds_scope.CapacityBar, {
    remaining: remaining,
    reading: reading,
    stale: stale
  })) : /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: state === "broken" ? "var(--text-secondary)" : "var(--text-tertiary)",
      lineHeight: "var(--leading-snug)"
    }
  }, state === "connecting" ? "Connecting…" : state === "idle" ? "Not connected — needs one more step." : noLimits ? "No limits reported — nothing to show yet." : reason || "Can’t read this subscription."), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)",
      minHeight: 20
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      flex: 1,
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      color: stale ? "var(--amber)" : "var(--text-tertiary)"
    }
  }, state === "reading" ? "Reading…" : state === "connecting" ? "This can take a few seconds" : lastRead ? `Last read ${lastRead}` : ""), actionLabel ? /*#__PURE__*/React.createElement(TinyBtn, {
    label: actionLabel,
    onClick: onAction
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--text-accent)",
      fontWeight: "var(--weight-medium)"
    }
  }, actionLabel)) : null, windows && windows.length > 0 ? /*#__PURE__*/React.createElement(TinyBtn, {
    label: expanded ? "Hide limits" : "Show limits",
    onClick: onToggleExpand
  }, windows.length, " ", windows.length === 1 ? "limit" : "limits", " ", /*#__PURE__*/React.createElement(Chevron, {
    open: expanded
  })) : null), expanded && windows && windows.length > 0 ? /*#__PURE__*/React.createElement("div", {
    style: {
      borderTop: "0.5px solid var(--border-subtle)",
      paddingTop: "var(--space-1)",
      marginTop: "var(--space-0-5)"
    }
  }, windows.map((w, i) => /*#__PURE__*/React.createElement(__ds_scope.LimitWindow, _extends({
    key: i
  }, w, {
    stale: stale,
    style: i < windows.length - 1 ? {
      borderBottom: "0.5px solid var(--border-subtle)"
    } : null
  })))) : null);
}
Object.assign(__ds_scope, { SubscriptionRow });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/subscription/SubscriptionRow.jsx", error: String((e && e.message) || e) }); }

// ui_kits/quotos/data.js
try { (() => {
/* Quotos UI kit — mock data & helpers (plain globals, no build step). */
(function () {
  // A realistic mixed-state set: working (incl. one critical), behind, waiting,
  // broken, not-connected. Two are pinned to the menu bar.
  window.QUOTOS_SEED = [{
    id: "max",
    label: "Claude Max",
    provider: "Anthropic",
    account: "Personal",
    state: "working",
    remaining: 62,
    resetLabel: "resets in 3h",
    lastRead: "2 min ago",
    pinned: true,
    windows: [{
      name: "Session",
      remaining: 62,
      resetLabel: "resets in 3h"
    }, {
      name: "Weekly",
      remaining: 41,
      resetLabel: "resets Mon",
      scope: "Opus 4"
    }, {
      name: "Code review",
      resetLabel: null
    }]
  }, {
    id: "repair",
    label: "Claude Max",
    provider: "Anthropic",
    account: "Personal · 2",
    state: "repairing",
    remaining: 71,
    resetLabel: "resets in 6h",
    lastRead: "5 min ago",
    pinned: false,
    windows: [{
      name: "Session",
      remaining: 71,
      resetLabel: "resets in 6h"
    }]
  }, {
    id: "team",
    label: "Claude Team",
    provider: "Anthropic",
    account: "Work",
    state: "working",
    remaining: 8,
    resetLabel: "resets in 90 min",
    lastRead: "2 min ago",
    pinned: true,
    windows: [{
      name: "Session",
      remaining: 8,
      resetLabel: "resets in 90 min"
    }, {
      name: "Weekly",
      remaining: 34,
      resetLabel: "resets Thu"
    }]
  }, {
    id: "eu",
    label: "Claude Team (EU)",
    provider: "Anthropic",
    account: "Work · Frankfurt",
    state: "behind",
    remaining: 40,
    resetLabel: "resets in 5h",
    lastRead: "41 min ago",
    pinned: false,
    windows: [{
      name: "Session",
      remaining: 40,
      resetLabel: "resets in 5h"
    }, {
      name: "Wöchentliches Limit",
      remaining: 71,
      resetLabel: null
    }]
  }, {
    id: "api",
    label: "Research key",
    provider: "Anthropic",
    account: "API",
    state: "waiting",
    remaining: 88,
    resetLabel: "resets weekly",
    lastRead: "just now",
    pinned: false,
    windows: [{
      name: "Weekly",
      remaining: 88,
      resetLabel: "resets Sun"
    }, {
      name: "Opus",
      remaining: 55,
      resetLabel: null,
      scope: "Opus 4"
    }, {
      name: "Sonnet",
      remaining: 92,
      resetLabel: null,
      scope: "Sonnet"
    }]
  }, {
    id: "flex",
    label: "Flex tier",
    provider: "Anthropic",
    account: "API",
    state: "working",
    remaining: null,
    resetLabel: null,
    lastRead: "just now",
    pinned: false,
    windows: []
  }, {
    id: "old",
    label: "Old account",
    provider: "Anthropic",
    account: "Personal",
    state: "broken",
    remaining: null,
    resetLabel: null,
    lastRead: "1h ago",
    reason: "Sign-in expired. Reconnect to resume reading.",
    actionLabel: "Reconnect",
    pinned: false,
    windows: []
  }, {
    id: "new",
    label: "Team seat",
    provider: "Anthropic",
    account: "Not connected",
    state: "idle",
    remaining: null,
    resetLabel: null,
    lastRead: null,
    actionLabel: "Finish setup",
    pinned: false,
    windows: []
  }];

  // The add-subscription flow content.
  window.QUOTOS_ADD = {
    found: [{
      id: "f1",
      label: "claude.ai",
      detail: "Signed in as you@studio.dev · Max"
    }, {
      id: "f2",
      label: "Claude Code CLI",
      detail: "Credentials found in keychain"
    }],
    providers: [{
      id: "anthropic",
      label: "Anthropic",
      detail: "Claude — Max, Team, API",
      available: true
    }, {
      id: "more",
      label: "More providers",
      detail: "Coming after the first version",
      available: false
    }],
    methods: [{
      id: "cli",
      label: "Use Claude Code credentials",
      detail: "Reuse the key the CLI already stores. No pasting.",
      recommended: true
    }, {
      id: "session",
      label: "Use claude.ai session",
      detail: "Read using your logged-in browser session."
    }, {
      id: "key",
      label: "Paste an API key",
      detail: "For an API subscription. Starts with sk-ant-."
    }],
    // A sample verified reading shown on the result step.
    verified: {
      label: "Claude Max",
      account: "Personal",
      provider: "Anthropic",
      remaining: 74,
      resetLabel: "resets in 4h",
      windows: [{
        name: "Session",
        remaining: 74,
        resetLabel: "resets in 4h"
      }, {
        name: "Weekly",
        remaining: 58,
        resetLabel: "resets Mon"
      }]
    }
  };

  // Nudge a percentage a little, clamped — used to make refresh feel live.
  window.quotosJitter = function (n) {
    if (typeof n !== "number") return n;
    return Math.max(1, Math.min(99, n - Math.floor(Math.random() * 3)));
  };
})();
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/quotos/data.js", error: String((e && e.message) || e) }); }

__ds_ns.Button = __ds_scope.Button;

__ds_ns.IconButton = __ds_scope.IconButton;

__ds_ns.TextField = __ds_scope.TextField;

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.CapacityBar = __ds_scope.CapacityBar;

__ds_ns.StatusDot = __ds_scope.StatusDot;

__ds_ns.QuotaGlyph = __ds_scope.QuotaGlyph;

__ds_ns.MenuBarTile = __ds_scope.MenuBarTile;

__ds_ns.Panel = __ds_scope.Panel;

__ds_ns.LimitWindow = __ds_scope.LimitWindow;

__ds_ns.SubscriptionRow = __ds_scope.SubscriptionRow;

})();
