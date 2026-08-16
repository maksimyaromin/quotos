import { Button } from "@design-system";

/** Stand-in for a just-removed row for five seconds. App.tsx owns the
 * timer. Adopted in place of a confirmation dialog: cheaper than a dialog,
 * and nothing mutates under the cursor while it is showing. */
export function UndoRow({ label, onUndo }: { label: string; onUndo: () => void }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-2)",
        padding: "var(--space-3)",
        borderRadius: "var(--radius-md)",
      }}
    >
      <span
        style={{
          flex: 1,
          fontFamily: "var(--font-sans)",
          fontSize: "var(--text-sm)",
          color: "var(--text-tertiary)",
        }}
      >
        {label} is no longer tracked
      </span>
      <Button variant="ghost" size="sm" onClick={onUndo}>
        Undo
      </Button>
    </div>
  );
}
