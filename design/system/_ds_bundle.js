/* @ds-bundle: {"format":4,"namespace":"QuotosDesignSystem_52e720","components":[{"name":"Button","sourcePath":"components/controls/Button.jsx"},{"name":"IconButton","sourcePath":"components/controls/IconButton.jsx"},{"name":"TextField","sourcePath":"components/controls/TextField.jsx"},{"name":"Badge","sourcePath":"components/indicators/Badge.jsx"},{"name":"CapacityBar","sourcePath":"components/indicators/CapacityBar.jsx"},{"name":"StatusDot","sourcePath":"components/indicators/StatusDot.jsx"},{"name":"QuotaGlyph","sourcePath":"components/shell/MenuBarTile.jsx"},{"name":"MenuBarTile","sourcePath":"components/shell/MenuBarTile.jsx"},{"name":"Panel","sourcePath":"components/shell/Panel.jsx"},{"name":"LimitWindow","sourcePath":"components/subscription/LimitWindow.jsx"},{"name":"SubscriptionRow","sourcePath":"components/subscription/SubscriptionRow.jsx"}],"sourceHashes":{"components/controls/Button.jsx":"15f1424afbe1","components/controls/IconButton.jsx":"2907f265ea8e","components/controls/TextField.jsx":"22e2995ba284","components/indicators/Badge.jsx":"1e403801f18a","components/indicators/CapacityBar.jsx":"8705fb70551c","components/indicators/StatusDot.jsx":"094dab599991","components/shell/MenuBarTile.jsx":"17cffbfc1978","components/shell/Panel.jsx":"73e3cbdc883e","components/subscription/LimitWindow.jsx":"cc9d7f71484b","components/subscription/SubscriptionRow.jsx":"f30119b8c16f","ui_kits/quotos/data.js":"5ad1ee382d31"},"inlinedExternals":[],"unexposedExports":[{"name":"capacityColor","sourcePath":"components/indicators/CapacityBar.jsx"},{"name":"PANEL_RADIUS","sourcePath":"components/shell/Panel.jsx"},{"name":"BEAK_BASE_HALF","sourcePath":"components/shell/Panel.jsx"},{"name":"BEAK_HEIGHT","sourcePath":"components/shell/Panel.jsx"},{"name":"NOTCH_RESERVE","sourcePath":"components/shell/Panel.jsx"},{"name":"buildPanelOutlinePath","sourcePath":"components/shell/Panel.jsx"}]} */

(() => {

const __ds_ns = (window.QuotosDesignSystem_52e720 = window.QuotosDesignSystem_52e720 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/controls/Button.jsx
try { (() => {
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
  sm: { height: "22px", padding: "0 var(--space-2)", fontSize: "var(--text-sm)" },
  base: { height: "var(--control-height)", padding: "0 var(--space-3)", fontSize: "var(--text-base)" },
  lg: { height: "var(--control-height-lg)", padding: "0 var(--space-4)", fontSize: "var(--text-md)" }
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
    ...hover && !disabled ? { background: hoverBg } : null,
    ...active && !disabled ? { transform: "translateY(0.5px)", opacity: 0.9 } : null,
    ...disabled ? { opacity: 0.4, cursor: "default" } : null,
    ...fullWidth ? { width: "100%" } : null,
    ...style
  };
  return /* @__PURE__ */ React.createElement(
    "button",
    {
      type: "button",
      disabled,
      onClick: disabled ? void 0 : onClick,
      onMouseEnter: () => setHover(true),
      onMouseLeave: () => {
        setHover(false);
        setActive(false);
      },
      onMouseDown: () => setActive(true),
      onMouseUp: () => setActive(false),
      style: composed,
      ...rest
    },
    icon ? /* @__PURE__ */ React.createElement("span", { style: { display: "inline-flex", width: 14, height: 14 } }, icon) : null,
    children
  );
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/Button.jsx", error: String((e && e.message) || e) }); }

// components/controls/IconButton.jsx
try { (() => {
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
  return /* @__PURE__ */ React.createElement(
    "button",
    {
      type: "button",
      "aria-label": label,
      title: label,
      disabled,
      onClick: disabled ? void 0 : onClick,
      onMouseEnter: () => setHover(true),
      onMouseLeave: () => setHover(false),
      style: composed,
      ...rest
    },
    /* @__PURE__ */ React.createElement("span", { style: { display: "inline-flex", width: Math.round(size * 0.6), height: Math.round(size * 0.6) } }, children)
  );
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/controls/TextField.jsx
try { (() => {
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
  return /* @__PURE__ */ React.createElement("label", { style: { display: "flex", flexDirection: "column", gap: "var(--space-1)", ...style } }, label ? /* @__PURE__ */ React.createElement("span", { style: {
    fontFamily: "var(--font-sans)",
    fontSize: "var(--text-sm)",
    color: "var(--text-secondary)"
  } }, label) : null, /* @__PURE__ */ React.createElement(
    "input",
    {
      value,
      placeholder,
      disabled,
      onChange,
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
      },
      ...rest
    }
  ));
}
Object.assign(__ds_scope, { TextField });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/controls/TextField.jsx", error: String((e && e.message) || e) }); }

// components/indicators/Badge.jsx
try { (() => {
const TONES = {
  neutral: { color: "var(--text-tertiary)", bg: "var(--bg-elevated)", border: "var(--border-default)" },
  accent: { color: "var(--text-accent)", bg: "var(--teal-muted)", border: "transparent" },
  warn: { color: "var(--amber)", bg: "var(--amber-muted)", border: "transparent" },
  danger: { color: "var(--red)", bg: "var(--red-muted)", border: "transparent" },
  info: { color: "var(--blue)", bg: "var(--blue-muted)", border: "transparent" }
};
function Badge({ tone = "neutral", children, style }) {
  const t = TONES[tone] || TONES.neutral;
  return /* @__PURE__ */ React.createElement(
    "span",
    {
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
    },
    children
  );
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/indicators/Badge.jsx", error: String((e && e.message) || e) }); }

// components/indicators/CapacityBar.jsx
try { (() => {
function capacityColor(used) {
  if (used >= 90) return "var(--cap-critical)";
  if (used >= 75) return "var(--cap-warn)";
  return "var(--cap-healthy)";
}
function severityColor(severity) {
  if (severity === "critical") return "var(--cap-critical)";
  if (severity === "warn") return "var(--cap-warn)";
  return "var(--cap-healthy)";
}
function CapacityBar({
  used = 0,
  reading = false,
  stale = false,
  severity = null,
  height,
  style
}) {
  const fill = Math.max(0, Math.min(100, used));
  const color = severity ? severityColor(severity) : capacityColor(used);
  return /* @__PURE__ */ React.createElement(
    "div",
    {
      style: {
        position: "relative",
        width: "100%",
        height: height || "var(--cap-bar-height)",
        background: "var(--cap-track)",
        borderRadius: "var(--radius-pill)",
        overflow: "hidden",
        ...style
      }
    },
    /* @__PURE__ */ React.createElement(
      "div",
      {
        style: {
          position: "absolute",
          inset: 0,
          width: `${fill}%`,
          background: color,
          borderRadius: "var(--radius-pill)",
          opacity: stale ? 0.4 : 1,
          transition: "width var(--dur-slow) var(--ease-out), background var(--dur-base), opacity var(--dur-base)"
        }
      }
    ),
    reading ? (
      // data-quotos-shimmer lets tokens/elevation.css hide this overlay
      // under prefers-reduced-motion, where a stopped gradient would sit
      // as a static white stripe instead of a moving glint.
      /* @__PURE__ */ React.createElement(
        "div",
        {
          "data-quotos-shimmer": "",
          style: {
            position: "absolute",
            inset: 0,
            background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.28), transparent)",
            backgroundSize: "40% 100%",
            backgroundRepeat: "no-repeat",
            animation: "quotos-shimmer 1.1s var(--ease-standard) infinite"
          }
        }
      )
    ) : null,
    /* @__PURE__ */ React.createElement("style", null, `@keyframes quotos-shimmer{0%{background-position:-40% 0}100%{background-position:140% 0}}`)
  );
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
  broken: "var(--status-broken)"
};
const PULSING = /* @__PURE__ */ new Set(["connecting", "reading"]);
function StatusDot({ state = "working", size = 7, style }) {
  const pulsing = PULSING.has(state);
  return /* @__PURE__ */ React.createElement(
    "span",
    {
      style: {
        display: "inline-block",
        width: size,
        height: size,
        borderRadius: "var(--radius-pill)",
        backgroundColor: STATE_COLOR[state] || "var(--status-idle)",
        boxShadow: state === "idle" ? "inset 0 0 0 1.5px var(--status-idle)" : "none",
        backgroundClip: state === "idle" ? "content-box" : "border-box",
        opacity: state === "idle" ? 0.9 : 1,
        animation: pulsing ? "quotos-pulse 1.4s var(--ease-standard) infinite" : "none",
        ...style
      }
    },
    /* @__PURE__ */ React.createElement("style", null, `@keyframes quotos-pulse{0%,100%{opacity:1}50%{opacity:0.35}}`)
  );
}
Object.assign(__ds_scope, { StatusDot });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/indicators/StatusDot.jsx", error: String((e && e.message) || e) }); }

// components/shell/MenuBarTile.jsx
try { (() => {
function QuotaGlyph({ size = 15 }) {
  return /* @__PURE__ */ React.createElement("svg", { width: size, height: size, viewBox: "0 0 16 16", fill: "none", style: { flex: "0 0 auto" } }, /* @__PURE__ */ React.createElement("circle", { cx: "8", cy: "8", r: "5.4", stroke: "currentColor", strokeWidth: "1.4", opacity: "0.28" }), /* @__PURE__ */ React.createElement("path", { d: "M4.46 12.02a5.4 5.4 0 1 1 7.08 0", stroke: "currentColor", strokeWidth: "1.9", strokeLinecap: "round" }));
}
const AttentionMark = () => /* @__PURE__ */ React.createElement(
  "svg",
  {
    width: "11",
    height: "11",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "2.2",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  },
  /* @__PURE__ */ React.createElement("path", { d: "M10.3 3.6 1.8 18a1.9 1.9 0 0 0 1.7 2.9h17a1.9 1.9 0 0 0 1.7-2.9L13.7 3.6a1.9 1.9 0 0 0-3.4 0Z" }),
  /* @__PURE__ */ React.createElement("path", { d: "M12 9v4M12 17h.01" })
);
function MenuBarTile({ pins = [], onClick, showStrip = true, style }) {
  const tintFor = (p) => {
    if (p.state === "broken" || p.state === "behind") return "var(--amber)";
    if (typeof p.used === "number" && p.used >= 90) return "var(--red)";
    if (typeof p.used === "number" && p.used >= 75) return "var(--amber)";
    return "inherit";
  };
  const tile = /* @__PURE__ */ React.createElement("button", { type: "button", onClick, style: {
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
  } }, /* @__PURE__ */ React.createElement(QuotaGlyph, null), pins.map((p, i) => /* @__PURE__ */ React.createElement("span", { key: i, style: { display: "inline-flex", alignItems: "center", gap: 3, color: tintFor(p) } }, p.state === "broken" ? /* @__PURE__ */ React.createElement(AttentionMark, null) : /* @__PURE__ */ React.createElement(React.Fragment, null, typeof p.used === "number" ? `${p.used}%` : "\u2014", p.state === "behind" ? /* @__PURE__ */ React.createElement("span", { style: { opacity: 0.7 } }, /* @__PURE__ */ React.createElement(AttentionMark, null)) : null))));
  if (!showStrip) return /* @__PURE__ */ React.createElement("span", { style }, tile);
  return /* @__PURE__ */ React.createElement("div", { style: {
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
  } }, tile, /* @__PURE__ */ React.createElement("span", { style: { display: "inline-flex", alignItems: "center", gap: 3, color: "rgba(255,255,255,0.6)", fontFamily: "var(--font-sans)", fontSize: 12 } }, "100%", /* @__PURE__ */ React.createElement("svg", { width: "22", height: "12", viewBox: "0 0 26 13", fill: "none" }, /* @__PURE__ */ React.createElement("rect", { x: "0.5", y: "0.5", width: "22", height: "12", rx: "3", stroke: "currentColor", opacity: "0.7" }), /* @__PURE__ */ React.createElement("rect", { x: "2", y: "2", width: "19", height: "9", rx: "1.5", fill: "currentColor" }), /* @__PURE__ */ React.createElement("rect", { x: "23.5", y: "4", width: "2", height: "5", rx: "1", fill: "currentColor", opacity: "0.7" }))), /* @__PURE__ */ React.createElement("span", { style: { color: "rgba(255,255,255,0.82)", fontFamily: "var(--font-sans)", fontSize: 12 } }, "Mon 9:41"));
}
Object.assign(__ds_scope, { QuotaGlyph, MenuBarTile });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/MenuBarTile.jsx", error: String((e && e.message) || e) }); }

// components/shell/Panel.jsx
try { (() => {
const { useId, useLayoutEffect, useRef, useState } = React;
const PANEL_WIDTH = 332;
const PANEL_RADIUS = 12;
const BEAK_BASE_HALF = 10;
const BEAK_HEIGHT = 10;
const BEAK_TIP_ROUND = 3;
const NOTCH_RESERVE = 12;
function buildPanelOutlinePath(width, height, beakLeft) {
  const rectTop = NOTCH_RESERVE;
  const rectBottom = NOTCH_RESERVE + height;
  let leftRadius = PANEL_RADIUS;
  let rightRadius = PANEL_RADIUS;
  if (beakLeft != null) {
    leftRadius = Math.max(0, Math.min(PANEL_RADIUS, beakLeft));
    rightRadius = Math.max(0, Math.min(PANEL_RADIUS, width - (beakLeft + BEAK_BASE_HALF * 2)));
  }
  const segments = [`M ${leftRadius} ${rectTop}`];
  if (beakLeft != null) {
    const baseLeftX = beakLeft;
    const baseRightX = beakLeft + BEAK_BASE_HALF * 2;
    const apexX = beakLeft + BEAK_BASE_HALF;
    const apexY = rectTop - BEAK_HEIGHT;
    const leftLen = Math.hypot(baseLeftX - apexX, rectTop - apexY);
    const p1x = apexX + (baseLeftX - apexX) / leftLen * BEAK_TIP_ROUND;
    const p1y = apexY + (rectTop - apexY) / leftLen * BEAK_TIP_ROUND;
    const rightLen = Math.hypot(baseRightX - apexX, rectTop - apexY);
    const p2x = apexX + (baseRightX - apexX) / rightLen * BEAK_TIP_ROUND;
    const p2y = apexY + (rectTop - apexY) / rightLen * BEAK_TIP_ROUND;
    segments.push(
      `L ${baseLeftX} ${rectTop}`,
      `L ${p1x} ${p1y}`,
      `Q ${apexX} ${apexY} ${p2x} ${p2y}`,
      `L ${baseRightX} ${rectTop}`
    );
  }
  const r = PANEL_RADIUS;
  segments.push(
    `L ${width - rightRadius} ${rectTop}`,
    `A ${rightRadius} ${rightRadius} 0 0 1 ${width} ${rectTop + rightRadius}`,
    `L ${width} ${rectBottom - r}`,
    `A ${r} ${r} 0 0 1 ${width - r} ${rectBottom}`,
    `L ${r} ${rectBottom}`,
    `A ${r} ${r} 0 0 1 0 ${rectBottom - r}`,
    `L 0 ${rectTop + leftRadius}`,
    `A ${leftRadius} ${leftRadius} 0 0 1 ${leftRadius} ${rectTop}`,
    "Z"
  );
  return segments.join(" ");
}
function Panel({
  title = "Quotos",
  docked = true,
  beakLeft = 24,
  dragging = false,
  onHeaderPointerDown,
  leading = null,
  headerActions = null,
  footer = null,
  maxBodyHeight = 452,
  position = null,
  children,
  style
}) {
  const detachedFixed = !docked && position ? { position: "fixed", left: position.x, top: position.y, margin: 0 } : null;
  const clipId = useId();
  const contentRef = useRef(null);
  const [contentHeight, setContentHeight] = useState(0);
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return void 0;
    setContentHeight(el.getBoundingClientRect().height);
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.height;
      if (typeof next === "number") setContentHeight(next);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const showBeak = docked && !detachedFixed;
  const pathD = contentHeight > 0 ? buildPanelOutlinePath(PANEL_WIDTH, contentHeight, showBeak ? beakLeft : null) : null;
  const beakBoxHeight = contentHeight + NOTCH_RESERVE;
  return /* @__PURE__ */ React.createElement("div", { "data-quotos-panel": "true", style: { position: "relative", width: "var(--panel-width)", ...detachedFixed, ...style } }, pathD ? /* @__PURE__ */ React.createElement(React.Fragment, null, /* @__PURE__ */ React.createElement("svg", { width: "0", height: "0", style: { position: "absolute" } }, /* @__PURE__ */ React.createElement("defs", null, /* @__PURE__ */ React.createElement("clipPath", { id: clipId }, /* @__PURE__ */ React.createElement("path", { d: pathD })))), /* @__PURE__ */ React.createElement(
    "div",
    {
      style: {
        position: "absolute",
        top: -NOTCH_RESERVE,
        left: 0,
        width: PANEL_WIDTH,
        height: beakBoxHeight,
        background: "var(--bg-panel)",
        backdropFilter: "var(--blur-vibrancy)",
        WebkitBackdropFilter: "var(--blur-vibrancy)",
        boxShadow: "var(--shadow-popover)",
        clipPath: `url(#${clipId})`,
        WebkitClipPath: `url(#${clipId})`
      }
    }
  )) : null, /* @__PURE__ */ React.createElement(
    "div",
    {
      ref: contentRef,
      style: {
        display: "flex",
        flexDirection: "column",
        position: "relative",
        borderRadius: "var(--radius-xl)",
        overflow: "hidden"
      }
    },
    /* @__PURE__ */ React.createElement(
      "div",
      {
        onMouseDown: onHeaderPointerDown,
        style: {
          display: "flex",
          alignItems: "center",
          gap: "var(--space-1)",
          padding: "var(--space-2-5) var(--space-2) var(--space-2-5) var(--space-3)",
          borderBottom: "0.5px solid var(--border-subtle)",
          cursor: dragging ? "grabbing" : "grab",
          userSelect: "none"
        }
      },
      leading,
      /* @__PURE__ */ React.createElement("span", { style: {
        flex: 1,
        fontFamily: "var(--font-sans)",
        fontSize: "var(--text-base)",
        fontWeight: "var(--weight-semibold)",
        color: "var(--text-primary)",
        letterSpacing: "var(--tracking-tight)"
      } }, title),
      headerActions
    ),
    /* @__PURE__ */ React.createElement("div", { className: "quotos-scroll", style: {
      display: "flex",
      flexDirection: "column",
      gap: "var(--space-0-5)",
      padding: "var(--space-1-5)",
      maxHeight: maxBodyHeight,
      overflowY: "auto"
    } }, children),
    footer ? /* @__PURE__ */ React.createElement("div", { style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)",
      padding: "var(--space-1-5) var(--space-2)",
      borderTop: "0.5px solid var(--border-subtle)"
    } }, footer) : null
  ), pathD ? (
    // The border — one stroke along the same path the fill was clipped
    // to, painted *after* (on top of) the content box so the content's
    // own edge — which sits almost exactly on the rect portion of this
    // same boundary — never covers half its width. Traces the beak's
    // two exposed edges and the rect's corners as a single continuous
    // line, with no seam at the join between them.
    /* @__PURE__ */ React.createElement(
      "svg",
      {
        "aria-hidden": "true",
        width: PANEL_WIDTH,
        height: beakBoxHeight,
        viewBox: `0 0 ${PANEL_WIDTH} ${beakBoxHeight}`,
        style: { position: "absolute", top: -NOTCH_RESERVE, left: 0, overflow: "visible", pointerEvents: "none" }
      },
      /* @__PURE__ */ React.createElement("path", { d: pathD, fill: "none", style: { stroke: "var(--border-strong)" }, strokeWidth: 0.5 })
    )
  ) : null);
}
Object.assign(__ds_scope, { PANEL_RADIUS, BEAK_BASE_HALF, BEAK_HEIGHT, NOTCH_RESERVE, buildPanelOutlinePath, Panel });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/Panel.jsx", error: String((e && e.message) || e) }); }

// components/subscription/LimitWindow.jsx
try { (() => {
const { CapacityBar } = __ds_scope;
const { Badge } = __ds_scope;
function numberColor(used) {
  if (used >= 90) return "var(--red)";
  if (used >= 75) return "var(--amber)";
  return "var(--text-primary)";
}
const PinGlyph = () => /* @__PURE__ */ React.createElement(
  "svg",
  {
    width: "12",
    height: "12",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "1.8",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  },
  /* @__PURE__ */ React.createElement("path", { d: "M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" })
);
function LimitWindow({
  id,
  name,
  used = null,
  resetLabel = null,
  scope = null,
  stale = false,
  pinned = false,
  onTogglePin,
  style
}) {
  const hasPct = typeof used === "number";
  return /* @__PURE__ */ React.createElement("div", { style: { display: "flex", flexDirection: "column", gap: "var(--space-1)", padding: "var(--space-1-5) 0", ...style } }, /* @__PURE__ */ React.createElement("div", { style: { display: "flex", alignItems: "center", gap: "var(--space-1-5)" } }, /* @__PURE__ */ React.createElement(
    "button",
    {
      type: "button",
      title: pinned ? "Remove from menu bar" : "Show in menu bar",
      "aria-label": pinned ? "Remove from menu bar" : "Show in menu bar",
      "aria-pressed": pinned,
      onClick: (e) => {
        e.stopPropagation();
        onTogglePin?.(id);
      },
      style: {
        flex: "0 0 auto",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: 20,
        height: 20,
        padding: 0,
        border: 0,
        borderRadius: "var(--radius-xs)",
        background: pinned ? "var(--bg-selected)" : "transparent",
        color: pinned ? "var(--text-accent)" : "var(--text-quaternary)",
        cursor: "pointer"
      }
    },
    /* @__PURE__ */ React.createElement(PinGlyph, null)
  ), /* @__PURE__ */ React.createElement("span", { style: {
    flex: 1,
    minWidth: 0,
    fontFamily: "var(--font-sans)",
    fontSize: "var(--text-sm)",
    color: "var(--text-secondary)",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
  } }, name), scope ? (
    // `text-overflow` only applies to block containers, and the Badge is
    // itself a flex container — on it, overflow:hidden hard-clips the tag
    // mid-character with no "…" ever drawn (verified live; R3-2's note
    // claiming otherwise mistook the clip for an ellipsis). So the badge
    // keeps only the layout constraints and an inner block span owns the
    // truncation.
    /* @__PURE__ */ React.createElement(Badge, { tone: "neutral", style: { flex: "0 1 auto", minWidth: 0, maxWidth: 120 } }, /* @__PURE__ */ React.createElement("span", { style: { display: "block", minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" } }, scope))
  ) : null, /* @__PURE__ */ React.createElement("span", { style: {
    fontFamily: "var(--font-mono)",
    fontSize: "var(--numeral-sm)",
    fontWeight: "var(--weight-medium)",
    fontVariantNumeric: "tabular-nums",
    color: hasPct ? numberColor(used) : "var(--text-quaternary)",
    minWidth: 34,
    textAlign: "right"
  } }, hasPct ? `${used}%` : "\u2014")), hasPct ? /* @__PURE__ */ React.createElement(CapacityBar, { used, stale, height: "3px" }) : null, resetLabel ? /* @__PURE__ */ React.createElement("span", { style: { fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingLeft: 26 } }, resetLabel) : null);
}
Object.assign(__ds_scope, { LimitWindow });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/subscription/LimitWindow.jsx", error: String((e && e.message) || e) }); }

// components/subscription/SubscriptionRow.jsx
try { (() => {
const { CapacityBar } = __ds_scope;
const { StatusDot } = __ds_scope;
const { Badge } = __ds_scope;
const { LimitWindow } = __ds_scope;
const Chevron = ({ open }) => /* @__PURE__ */ React.createElement(
  "svg",
  {
    width: "12",
    height: "12",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "2.2",
    strokeLinecap: "round",
    strokeLinejoin: "round",
    style: { transform: open ? "rotate(180deg)" : "none", transition: "transform var(--dur-base) var(--ease-standard)" }
  },
  /* @__PURE__ */ React.createElement("path", { d: "M6 9l6 6 6-6" })
);
const PinGlyph = ({ size = 12 }) => /* @__PURE__ */ React.createElement(
  "svg",
  {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "1.8",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  },
  /* @__PURE__ */ React.createElement("path", { d: "M12 17v5M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .59 1.41l1.3 1.3A1 1 0 0 1 17.18 15H6.82a1 1 0 0 1-.7-1.71l1.29-1.32A2 2 0 0 0 9 10.76Z" })
);
const MenuDotsGlyph = () => /* @__PURE__ */ React.createElement("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "currentColor" }, /* @__PURE__ */ React.createElement("circle", { cx: "5", cy: "12", r: "1.6" }), /* @__PURE__ */ React.createElement("circle", { cx: "12", cy: "12", r: "1.6" }), /* @__PURE__ */ React.createElement("circle", { cx: "19", cy: "12", r: "1.6" }));
const STATE_DOT_COLOR = {
  working: "var(--status-working)",
  behind: "var(--status-behind)",
  broken: "var(--status-broken)"
};
const MENU_GAP = 4;
const MENU_VIEWPORT_MARGIN = 8;
const MENU_MIN_WIDTH = 168;
function MenuItem({ danger, disabled, onClick, children }) {
  const [hover, setHover] = React.useState(false);
  return /* @__PURE__ */ React.createElement(
    "button",
    {
      type: "button",
      role: "menuitem",
      disabled,
      onClick,
      onMouseEnter: () => setHover(true),
      onMouseLeave: () => setHover(false),
      style: {
        display: "flex",
        alignItems: "center",
        height: 26,
        padding: "0 var(--space-2)",
        border: 0,
        borderRadius: "var(--radius-sm)",
        background: hover && !disabled ? "var(--bg-row-hover)" : "transparent",
        fontFamily: "var(--font-sans)",
        fontSize: "var(--text-base)",
        color: disabled ? "var(--text-quaternary)" : danger ? "var(--red)" : "var(--text-primary)",
        textAlign: "left",
        cursor: disabled ? "default" : "pointer"
      }
    },
    children
  );
}
function SubscriptionRow({
  label,
  provider,
  account,
  state = "working",
  used = null,
  severity = "healthy",
  resetLabel = null,
  lastRead = null,
  windows = [],
  reason = null,
  /** R3-4: classified by the caller (see lib/rowPresentation.ts), not
   *  inferred from `state` here. "Needs sign-in" used to be shown for every
   *  `broken` row, so an offline launch or an HTTP 403 told the captain his
   *  working account was signed out. */
  badge = null,
  /** v4: how many of this subscription's windows are currently pinned — an
   * indicator, not a control (design/NOTES.md §2). Renders nothing at 0. */
  pinnedCount = 0,
  /** v4: whether the *headline* window specifically is pinned — what the
   * "…" menu's wording (below) reflects and toggles via `onTogglePin`. */
  headlinePinned = false,
  expanded = false,
  menuOpen = false,
  actionLabel = null,
  actionDisabled = false,
  footerNote = null,
  onAction,
  /** v4: toggles the *headline* window's pin — the "…" menu item only. */
  onTogglePin,
  /** v4: toggles a specific window's pin, called with that window's `id` —
   * wired to each row in the expanded list's own pin button. */
  onToggleWindowPin,
  onToggleExpand,
  onToggleMenu,
  onRename,
  onReadNow,
  /** v5: reordering — the "…" menu's "Move up"/"Move down". Panel order
   * drives the tray's digit order too, so this is how the person controls
   * which account's numbers come first. An edge row's impossible direction
   * renders disabled (macOS-style) rather than hidden, keeping the menu's
   * one fixed set of items. */
  canMoveUp = false,
  canMoveDown = false,
  onMoveUp,
  onMoveDown,
  onStopTracking,
  signInInProgress = false,
  onSubmitSignInCode,
  onCancelSignIn,
  style
}) {
  const [renaming, setRenaming] = React.useState(false);
  const [draft, setDraft] = React.useState(label);
  const inputRef = React.useRef(null);
  const [codeDraft, setCodeDraft] = React.useState("");
  const codeInputRef = React.useRef(null);
  const menuButtonRef = React.useRef(null);
  const menuRef = React.useRef(null);
  const [menuPos, setMenuPos] = React.useState(null);
  React.useLayoutEffect(() => {
    if (!menuOpen) {
      setMenuPos(null);
      return void 0;
    }
    const place = () => {
      const trigger = menuButtonRef.current;
      const menu = menuRef.current;
      if (!trigger || !menu) return;
      const anchor = trigger.getBoundingClientRect();
      const width = menu.offsetWidth || MENU_MIN_WIDTH;
      const height = menu.offsetHeight;
      const below = anchor.bottom + MENU_GAP;
      const top = below + height <= window.innerHeight - MENU_VIEWPORT_MARGIN ? below : Math.max(MENU_VIEWPORT_MARGIN, anchor.top - MENU_GAP - height);
      const left = Math.min(
        Math.max(MENU_VIEWPORT_MARGIN, anchor.right - width),
        Math.max(MENU_VIEWPORT_MARGIN, window.innerWidth - width - MENU_VIEWPORT_MARGIN)
      );
      setMenuPos((prev) => prev && prev.top === top && prev.left === left ? prev : { top, left });
    };
    place();
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [menuOpen]);
  const wasMenuOpen = React.useRef(false);
  React.useEffect(() => {
    if (wasMenuOpen.current && !menuOpen && document.activeElement === document.body) {
      menuButtonRef.current?.focus();
    }
    wasMenuOpen.current = menuOpen;
  }, [menuOpen]);
  React.useEffect(() => {
    if (signInInProgress) {
      setCodeDraft("");
      requestAnimationFrame(() => codeInputRef.current?.focus());
    }
  }, [signInInProgress]);
  const submitCode = () => {
    const trimmed = codeDraft.trim();
    if (trimmed.length === 0) return;
    onSubmitSignInCode?.(trimmed);
    setCodeDraft("");
  };
  React.useEffect(() => {
    if (renaming) {
      setDraft(label);
      requestAnimationFrame(() => inputRef.current?.select());
    }
  }, [renaming, label]);
  const commitRename = () => {
    setRenaming(false);
    const trimmed = draft.trim();
    if (trimmed === label) return;
    onRename?.(trimmed.length > 0 ? trimmed : null);
  };
  const stale = state === "behind";
  const reading = state === "reading" || state === "connecting";
  const hasData = typeof used === "number";
  const numColor = stale ? "var(--amber)" : severity === "critical" ? "var(--red)" : severity === "warn" ? "var(--amber)" : "var(--text-primary)";
  const dotColor = STATE_DOT_COLOR[stale ? "behind" : state] || "var(--status-progress)";
  const handleRowClick = () => {
    if (menuOpen) {
      onToggleMenu?.();
      return;
    }
    if (renaming) return;
    onToggleExpand?.();
  };
  const handleMenuKeyDown = (e) => {
    if (!menuOpen || !menuRef.current) return;
    if (e.target.tagName === "INPUT") return;
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
    const items = Array.from(menuRef.current.querySelectorAll("button")).filter((b) => !b.disabled);
    if (items.length === 0) return;
    e.preventDefault();
    const current = items.indexOf(document.activeElement);
    const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : e.key === "ArrowDown" ? current < 0 ? 0 : (current + 1) % items.length : current < 0 ? items.length - 1 : (current - 1 + items.length) % items.length;
    items[next].focus();
  };
  return /* @__PURE__ */ React.createElement(
    "div",
    {
      onClick: handleRowClick,
      onKeyDown: handleMenuKeyDown,
      style: {
        position: "relative",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-2)",
        padding: "var(--space-3)",
        borderRadius: "var(--radius-md)",
        background: expanded || menuOpen ? "var(--bg-row-hover)" : "transparent",
        cursor: "pointer",
        transition: "background var(--dur-fast) var(--ease-standard)",
        ...style
      }
    },
    /* @__PURE__ */ React.createElement("div", { style: { display: "flex", alignItems: "flex-start", gap: "var(--space-2)" } }, /* @__PURE__ */ React.createElement(StatusDot, { state, style: { marginTop: 5, flex: "0 0 auto" } }), /* @__PURE__ */ React.createElement("div", { style: { flex: 1, minWidth: 0 } }, renaming ? /* @__PURE__ */ React.createElement(
      "input",
      {
        ref: inputRef,
        value: draft,
        onClick: (e) => e.stopPropagation(),
        onChange: (e) => setDraft(e.target.value),
        onBlur: commitRename,
        onKeyDown: (e) => {
          if (e.key === "Enter") commitRename();
          if (e.key === "Escape") {
            e.stopPropagation();
            setDraft(label);
            setRenaming(false);
          }
        },
        style: {
          width: "100%",
          font: "inherit",
          fontFamily: "var(--font-sans)",
          fontSize: "var(--text-md)",
          fontWeight: "var(--weight-semibold)",
          color: "var(--text-primary)",
          letterSpacing: "var(--tracking-tight)",
          background: "var(--bg-input)",
          border: "0.5px solid var(--border-focus)",
          borderRadius: "var(--radius-xs)",
          padding: "1px 4px",
          margin: "-1px -4px",
          outline: "none"
        }
      }
    ) : /* @__PURE__ */ React.createElement("div", { style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-md)",
      fontWeight: "var(--weight-semibold)",
      color: "var(--text-primary)",
      letterSpacing: "var(--tracking-tight)",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    } }, label), provider || account ? /* @__PURE__ */ React.createElement("div", { style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      color: "var(--text-tertiary)",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    } }, [account, provider].filter(Boolean).join(" \xB7 ")) : null), pinnedCount > 0 ? /* @__PURE__ */ React.createElement(Badge, { tone: "accent", style: { flex: "0 0 auto", marginTop: 2, gap: 3 } }, /* @__PURE__ */ React.createElement(PinGlyph, { size: 10 }), pinnedCount) : null, badge ? /* @__PURE__ */ React.createElement(Badge, { tone: stale ? "warn" : "danger", style: { marginTop: 2, flex: "0 0 auto" } }, badge) : null, /* @__PURE__ */ React.createElement(
      "button",
      {
        type: "button",
        title: "More",
        "aria-label": "More",
        "aria-haspopup": "menu",
        "aria-expanded": menuOpen,
        "data-quotos-menu-scope": "true",
        ref: menuButtonRef,
        onClick: (e) => {
          e.stopPropagation();
          onToggleMenu?.();
        },
        style: {
          flex: "0 0 auto",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: 20,
          height: 20,
          margin: "1px -4px 0 0",
          padding: 0,
          border: 0,
          borderRadius: "var(--radius-xs)",
          background: menuOpen ? "var(--bg-row-hover)" : "transparent",
          color: menuOpen ? "var(--text-primary)" : "var(--text-quaternary)",
          cursor: "pointer"
        }
      },
      /* @__PURE__ */ React.createElement(MenuDotsGlyph, null)
    )),
    signInInProgress ? (
      // R2-6: Claude Code's own sign-in is running for this account (see
      // signin.rs) — it opens the browser itself, so Quotos only needs to
      // relay whatever code comes back. Replaces the reason text while
      // active; the row's own action button is hidden by the caller.
      /* @__PURE__ */ React.createElement("div", { style: { display: "flex", flexDirection: "column", gap: "var(--space-2)" } }, /* @__PURE__ */ React.createElement("div", { style: {
        fontFamily: "var(--font-sans)",
        fontSize: "var(--text-sm)",
        lineHeight: "var(--leading-snug)",
        color: "var(--text-secondary)"
      } }, "Finish signing in in the browser, then paste the code here."), /* @__PURE__ */ React.createElement("div", { style: { display: "flex", gap: "var(--space-2)" }, onClick: (e) => e.stopPropagation() }, /* @__PURE__ */ React.createElement(
        "input",
        {
          ref: codeInputRef,
          value: codeDraft,
          onChange: (e) => setCodeDraft(e.target.value),
          onKeyDown: (e) => {
            if (e.key === "Enter") submitCode();
            if (e.key === "Escape") {
              e.stopPropagation();
              setCodeDraft("");
              onCancelSignIn?.();
            }
          },
          placeholder: "Paste code",
          style: {
            flex: 1,
            font: "inherit",
            fontFamily: "var(--font-mono)",
            fontSize: "var(--text-sm)",
            color: "var(--text-primary)",
            background: "var(--bg-input)",
            border: "0.5px solid var(--border-focus)",
            borderRadius: "var(--radius-xs)",
            padding: "3px 6px",
            outline: "none"
          }
        }
      ), /* @__PURE__ */ React.createElement(
        "button",
        {
          type: "button",
          onClick: submitCode,
          disabled: codeDraft.trim().length === 0,
          style: {
            padding: "0 10px",
            border: 0,
            borderRadius: "var(--radius-xs)",
            background: "var(--bg-selected)",
            color: codeDraft.trim().length === 0 ? "var(--text-quaternary)" : "var(--text-accent)",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-sm)",
            fontWeight: "var(--weight-medium)",
            cursor: codeDraft.trim().length === 0 ? "default" : "pointer"
          }
        },
        "Submit"
      ), /* @__PURE__ */ React.createElement(
        "button",
        {
          type: "button",
          onClick: () => onCancelSignIn?.(),
          style: {
            padding: "0 8px",
            border: 0,
            borderRadius: "var(--radius-xs)",
            background: "transparent",
            color: "var(--text-tertiary)",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-sm)",
            cursor: "pointer"
          }
        },
        "Cancel"
      )))
    ) : hasData ? /* @__PURE__ */ React.createElement(React.Fragment, null, /* @__PURE__ */ React.createElement("div", { style: { display: "flex", alignItems: "flex-end", gap: "var(--space-2)" } }, /* @__PURE__ */ React.createElement("div", { style: { display: "flex", alignItems: "baseline", gap: "var(--space-1)", flex: 1 } }, /* @__PURE__ */ React.createElement("span", { style: {
      fontFamily: "var(--font-mono)",
      fontSize: "var(--numeral-lg)",
      fontWeight: "var(--weight-medium)",
      fontVariantNumeric: "tabular-nums",
      lineHeight: 1,
      color: numColor,
      letterSpacing: "var(--tracking-tighter)"
    } }, used, /* @__PURE__ */ React.createElement("span", { style: { fontSize: "18px" } }, "%")), /* @__PURE__ */ React.createElement("span", { style: { fontFamily: "var(--font-sans)", fontSize: "var(--text-sm)", color: "var(--text-tertiary)" } }, "used")), resetLabel ? /* @__PURE__ */ React.createElement("span", { style: { fontFamily: "var(--font-sans)", fontSize: "var(--text-xs)", color: "var(--text-tertiary)", paddingBottom: 3 } }, resetLabel) : null), /* @__PURE__ */ React.createElement(CapacityBar, { used, reading, stale, severity })) : /* @__PURE__ */ React.createElement("div", { style: {
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-sm)",
      lineHeight: "var(--leading-snug)",
      color: "var(--text-secondary)"
    } }, reason || "No limits reported yet."),
    /* @__PURE__ */ React.createElement("div", { style: { display: "flex", alignItems: "center", gap: "var(--space-2)", minHeight: 20 } }, /* @__PURE__ */ React.createElement("span", { style: {
      flex: 1,
      fontFamily: "var(--font-sans)",
      fontSize: "var(--text-xs)",
      color: stale ? "var(--amber)" : "var(--text-tertiary)"
    } }, reading ? "Reading\u2026" : footerNote ? footerNote : lastRead ? `Read ${lastRead}` : "Not read yet"), actionLabel ? /* @__PURE__ */ React.createElement(
      "button",
      {
        type: "button",
        title: actionLabel,
        onClick: (e) => {
          e.stopPropagation();
          if (!actionDisabled) onAction?.();
        },
        style: {
          height: 20,
          padding: "0 6px",
          border: 0,
          borderRadius: "var(--radius-xs)",
          background: "transparent",
          fontFamily: "var(--font-sans)",
          fontSize: "var(--text-xs)",
          fontWeight: "var(--weight-medium)",
          cursor: actionDisabled ? "default" : "pointer",
          color: actionDisabled ? "var(--text-quaternary)" : "var(--text-accent)"
        }
      },
      actionLabel
    ) : null, windows && windows.length > 0 ? (
      // R6: a real button, not a span — the row div's own onClick is the
      // pointer path, but a div is unreachable by keyboard and invisible
      // to the accessibility tree, so this is the row's only focusable
      // expand control. stopPropagation keeps the row's click from
      // toggling it straight back.
      /* @__PURE__ */ React.createElement(
        "button",
        {
          type: "button",
          "aria-expanded": expanded,
          onClick: (e) => {
            e.stopPropagation();
            onToggleExpand?.();
          },
          style: {
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            margin: 0,
            padding: 0,
            border: 0,
            background: "transparent",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-xs)",
            color: "var(--text-tertiary)",
            cursor: "pointer"
          }
        },
        windows.length,
        " ",
        windows.length === 1 ? "limit" : "limits",
        /* @__PURE__ */ React.createElement(Chevron, { open: expanded })
      )
    ) : null),
    /* @__PURE__ */ React.createElement(
      "div",
      {
        "aria-hidden": !expanded,
        style: {
          display: "grid",
          gridTemplateRows: expanded && windows && windows.length > 0 ? "1fr" : "0fr",
          transition: "grid-template-rows var(--dur-base) var(--ease-standard)"
        }
      },
      /* @__PURE__ */ React.createElement("div", { style: { overflow: "hidden", minHeight: 0 } }, windows && windows.length > 0 ? /* @__PURE__ */ React.createElement("div", { style: {
        borderTop: "0.5px solid var(--border-subtle)",
        paddingTop: "var(--space-1)",
        marginTop: "var(--space-0-5)"
      } }, windows.map((w, i) => /* @__PURE__ */ React.createElement(
        LimitWindow,
        {
          key: w.id ?? i,
          ...w,
          stale,
          onTogglePin: onToggleWindowPin,
          style: i < windows.length - 1 ? { borderBottom: "0.5px solid var(--border-subtle)" } : null
        }
      ))) : null)
    ),
    menuOpen ? /* @__PURE__ */ React.createElement(
      "div",
      {
        ref: menuRef,
        role: "menu",
        "aria-label": "Subscription actions",
        "data-quotos-menu-scope": "true",
        onClick: (e) => e.stopPropagation(),
        style: {
          position: "fixed",
          top: menuPos ? menuPos.top : 0,
          left: menuPos ? menuPos.left : 0,
          visibility: menuPos ? "visible" : "hidden",
          zIndex: 30,
          minWidth: MENU_MIN_WIDTH,
          padding: "var(--space-1)",
          borderRadius: "var(--radius-lg)",
          background: "var(--bg-elevated)",
          border: "0.5px solid var(--border-default)",
          boxShadow: "var(--shadow-menu)",
          display: "flex",
          flexDirection: "column"
        }
      },
      /* @__PURE__ */ React.createElement(MenuItem, { onClick: () => {
        onToggleMenu?.();
        onReadNow?.();
      } }, "Read now"),
      /* @__PURE__ */ React.createElement(MenuItem, { onClick: () => {
        onToggleMenu?.();
        setRenaming(true);
      } }, "Rename"),
      /* @__PURE__ */ React.createElement(MenuItem, { onClick: () => {
        onToggleMenu?.();
        onTogglePin?.();
      } }, headlinePinned ? "Hide from menu bar" : "Show in menu bar"),
      /* @__PURE__ */ React.createElement(MenuItem, { disabled: !canMoveUp, onClick: () => {
        onToggleMenu?.();
        onMoveUp?.();
      } }, "Move up"),
      /* @__PURE__ */ React.createElement(MenuItem, { disabled: !canMoveDown, onClick: () => {
        onToggleMenu?.();
        onMoveDown?.();
      } }, "Move down"),
      /* @__PURE__ */ React.createElement("div", { role: "separator", style: { height: "0.5px", margin: "var(--space-1) var(--space-2)", background: "var(--border-default)" } }),
      /* @__PURE__ */ React.createElement(MenuItem, { danger: true, onClick: () => {
        onToggleMenu?.();
        onStopTracking?.();
      } }, "Stop tracking")
    ) : null
  );
}
Object.assign(__ds_scope, { SubscriptionRow });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/subscription/SubscriptionRow.jsx", error: String((e && e.message) || e) }); }

// ui_kits/quotos/data.js
try { (() => {
(function() {
  window.QUOTOS_SEED = [
    {
      id: "max",
      label: "Claude Max",
      provider: "Anthropic",
      account: "Personal",
      state: "working",
      used: 38,
      severity: "warn",
      resetLabel: "Resets today at 4:05 PM",
      lastRead: "2 min ago",
      pinnedCount: 1,
      headlinePinned: true,
      windows: [
        { id: "max-session", name: "Session", used: 38, resetLabel: "Resets today at 4:05 PM", pinned: true },
        { id: "max-weekly", name: "Weekly", used: 82, resetLabel: "Resets Mon", scope: "Opus 4" },
        { id: "max-review", name: "Code review", resetLabel: null }
      ]
    },
    {
      id: "second",
      label: "Claude Max",
      provider: "Anthropic",
      account: "Personal \xB7 2",
      state: "reading",
      used: 29,
      severity: "healthy",
      resetLabel: "Resets in 6h",
      lastRead: "5 min ago",
      pinnedCount: 0,
      headlinePinned: false,
      windows: [
        { id: "second-session", name: "Session", used: 29, resetLabel: "Resets in 6h" }
      ]
    },
    {
      id: "team",
      label: "Claude Team",
      provider: "Anthropic",
      account: "Work",
      state: "working",
      used: 92,
      severity: "critical",
      resetLabel: "Resets in 90 min",
      lastRead: "2 min ago",
      pinnedCount: 1,
      headlinePinned: true,
      windows: [
        { id: "team-session", name: "Session", used: 92, resetLabel: "Resets in 90 min", pinned: true },
        { id: "team-weekly", name: "Weekly", used: 66, resetLabel: "Resets Thu" }
      ]
    },
    {
      id: "eu",
      label: "Claude Team (EU)",
      provider: "Anthropic",
      account: "Work \xB7 Frankfurt",
      state: "behind",
      used: 60,
      severity: "healthy",
      resetLabel: "Resets in 5h",
      lastRead: "41 min ago",
      badge: "Not current",
      actionLabel: "Try again",
      pinnedCount: 0,
      headlinePinned: false,
      windows: [
        { id: "eu-session", name: "Session", used: 60, resetLabel: "Resets in 5h" },
        { id: "eu-weekly", name: "W\xF6chentliches Limit", used: 29, resetLabel: null }
      ]
    },
    {
      id: "api",
      label: "Research key",
      provider: "Anthropic",
      account: "API",
      state: "working",
      used: 12,
      severity: "healthy",
      resetLabel: "Resets Sun",
      lastRead: "just now",
      footerNote: "Waiting for the rate budget \xB7 40 s",
      pinnedCount: 0,
      headlinePinned: false,
      windows: [
        { id: "api-weekly", name: "Weekly", used: 12, resetLabel: "Resets Sun" },
        { id: "api-opus", name: "Opus", used: 45, resetLabel: null, scope: "Opus 4" },
        { id: "api-sonnet", name: "Sonnet", used: 8, resetLabel: null, scope: "Sonnet" }
      ]
    },
    {
      id: "flex",
      label: "Flex tier",
      provider: "Anthropic",
      account: "API",
      state: "working",
      used: null,
      severity: "healthy",
      resetLabel: null,
      lastRead: "just now",
      pinnedCount: 0,
      headlinePinned: false,
      windows: []
    },
    {
      id: "old",
      label: "Old account",
      provider: "Anthropic",
      account: "Personal",
      state: "broken",
      used: null,
      resetLabel: null,
      lastRead: "1h ago",
      reason: "Sign-in expired. Sign in to resume reading.",
      badge: "Needs sign-in",
      actionLabel: "Sign in",
      pinnedCount: 0,
      headlinePinned: false,
      windows: []
    },
    {
      id: "new",
      label: "Team seat",
      provider: "Anthropic",
      account: "Not connected",
      state: "idle",
      used: null,
      resetLabel: null,
      lastRead: null,
      actionLabel: "Finish setup",
      pinnedCount: 0,
      headlinePinned: false,
      windows: []
    }
  ];
  window.QUOTOS_ADD = {
    found: [
      { id: "f1", label: "claude.ai", detail: "Signed in as you@studio.dev \xB7 Max" },
      { id: "f2", label: "Claude Code CLI", detail: "Credentials found in keychain" }
    ],
    providers: [
      { id: "anthropic", label: "Anthropic", detail: "Claude \u2014 Max, Team, API", available: true },
      { id: "more", label: "More providers", detail: "Coming after the first version", available: false }
    ],
    methods: [
      { id: "cli", label: "Use Claude Code credentials", detail: "Reuse the key the CLI already stores. No pasting.", recommended: true },
      { id: "session", label: "Use claude.ai session", detail: "Read using your logged-in browser session." },
      { id: "key", label: "Paste an API key", detail: "For an API subscription. Starts with sk-ant-." }
    ],
    // A sample verified reading shown on the result step.
    verified: {
      label: "Claude Max",
      account: "Personal",
      provider: "Anthropic",
      used: 26,
      resetLabel: "resets in 4h",
      windows: [
        { id: "v-session", name: "Session", used: 26, resetLabel: "Resets in 4h" },
        { id: "v-weekly", name: "Weekly", used: 42, resetLabel: "Resets Mon" }
      ]
    }
  };
  window.quotosJitter = function(n) {
    if (typeof n !== "number") return n;
    return Math.max(1, Math.min(99, n + Math.floor(Math.random() * 3)));
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
