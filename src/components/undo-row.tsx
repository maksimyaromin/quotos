import { Button } from "@/design-system";
import styles from "./undo-row.module.css";

/** Stand-in for a just-removed row for five seconds. app.tsx owns the
 * timer. Adopted in place of a confirmation dialog: cheaper than a dialog,
 * and nothing mutates under the cursor while it is showing. */
export function UndoRow({ label, onUndo }: { label: string; onUndo: () => void }) {
  return (
    <div className={styles.row}>
      <span className={styles.label}>{label} is no longer tracked</span>
      <Button variant="ghost" size="sm" onClick={onUndo}>
        Undo
      </Button>
    </div>
  );
}
