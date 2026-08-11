import React from "react";

/** The popover shell: a floating macOS vibrancy surface with a top beak, a
 *  header (title + toolbar actions), a scrollable body, and an optional footer.
 *  Depth is a single float — a soft shadow + hairline rim. */
export function Panel({
  title = "Quotos",
  beak = true,
  headerActions = null,
  footer = null,
  maxBodyHeight = 460,
  children,
  style,
}) {
  return (
    <div style={{ position: "relative", width: "var(--panel-width)", ...style }}>
      {beak ? (
        <div style={{
          position: "absolute", top: -6, left: "50%", transform: "translateX(-50%) rotate(45deg)",
          width: 12, height: 12, background: "var(--bg-panel)",
          borderTop: "0.5px solid var(--border-strong)", borderLeft: "0.5px solid var(--border-strong)",
          borderTopLeftRadius: 3, zIndex: 2,
        }} />
      ) : null}
      <div style={{
        display: "flex", flexDirection: "column",
        background: "var(--bg-panel)",
        backdropFilter: "var(--blur-vibrancy)",
        WebkitBackdropFilter: "var(--blur-vibrancy)",
        border: "0.5px solid var(--border-strong)",
        borderRadius: "var(--radius-xl)",
        boxShadow: "var(--shadow-popover)",
        overflow: "hidden",
      }}>
        {/* header */}
        <div style={{
          display: "flex", alignItems: "center", gap: "var(--space-2)",
          padding: "var(--space-2-5) var(--space-3)",
          borderBottom: "0.5px solid var(--border-subtle)",
        }}>
          <span style={{
            flex: 1, fontFamily: "var(--font-sans)", fontSize: "var(--text-base)",
            fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
            letterSpacing: "var(--tracking-tight)",
          }}>{title}</span>
          {headerActions}
        </div>
        {/* body */}
        <div style={{
          display: "flex", flexDirection: "column", gap: "var(--space-0-5)",
          padding: "var(--space-1-5)",
          maxHeight: maxBodyHeight, overflowY: "auto",
        }}>
          {children}
        </div>
        {/* footer */}
        {footer ? (
          <div style={{
            display: "flex", alignItems: "center", gap: "var(--space-2)",
            padding: "var(--space-2) var(--space-3)",
            borderTop: "0.5px solid var(--border-subtle)",
          }}>{footer}</div>
        ) : null}
      </div>
    </div>
  );
}
