import { Button } from "@/design-system";
import styles from "./undo-row.module.css";

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
