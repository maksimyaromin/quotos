import React from "react";

/** The popover shell: a floating macOS vibrancy surface with a top beak, a
 *  header (title + toolbar actions), a scrollable body, and an optional footer.
 *  Depth is a single float — a soft shadow + hairline rim.
 *
 *  The beak is pinned to the panel's left edge, not centered — the panel
 *  opens rightward from the tray icon, and `beakLeft` places the beak under
 *  wherever the glyph actually sits (mirrors the interactive prototype's own
 *  anchor math: `glyphCenterX - panelLeft`, clamped to stay inside the
 *  panel). Docked (attached under the tray) shows the beak; detached hides
 *  it and the header carries a snap-back affordance instead (see App.tsx).
 *
 *  The header is always grab/grabbing — dragging it is how the panel
 *  detaches (there is no detach button). `onHeaderPointerDown` is wired by
 *  the caller, which owns the drag-vs-click distinction and the actual
 *  window move (native Tauri drag, or a simulated position in the browser
 *  harness — see App.tsx). */
export function Panel({
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
  style,
}) {
  const detachedFixed = !docked && position ? { position: "fixed", left: position.x, top: position.y, margin: 0 } : null;
  return (
    <div data-quotos-panel="true" style={{ position: "relative", width: "var(--panel-width)", ...detachedFixed, ...style }}>
      {docked ? (
        <div style={{
          position: "absolute", top: -5, left: beakLeft, transform: "rotate(45deg)",
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
        {/* header — always the drag handle; the first movement detaches
            (see App.tsx's pointer handling), so the cursor always reads
            grab/grabbing even while docked. */}
        <div
          onMouseDown={onHeaderPointerDown}
          style={{
            display: "flex", alignItems: "center", gap: "var(--space-1)",
            padding: "var(--space-2-5) var(--space-2) var(--space-2-5) var(--space-3)",
            borderBottom: "0.5px solid var(--border-subtle)",
            cursor: dragging ? "grabbing" : "grab",
            userSelect: "none",
          }}
        >
          {leading}
          <span style={{
            flex: 1, fontFamily: "var(--font-sans)", fontSize: "var(--text-base)",
            fontWeight: "var(--weight-semibold)", color: "var(--text-primary)",
            letterSpacing: "var(--tracking-tight)",
          }}>{title}</span>
          {headerActions}
        </div>
        {/* body — no reserved scrollbar gutter; the track is hidden outright
            (scrollbar-width: none / ::-webkit-scrollbar{width:0} in app.css)
            so there is nothing to reserve space for in the first place. */}
        <div className="quotos-scroll" style={{
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
            padding: "var(--space-1-5) var(--space-2)",
            borderTop: "0.5px solid var(--border-subtle)",
          }}>{footer}</div>
        ) : null}
      </div>
    </div>
  );
}
