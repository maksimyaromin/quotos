import styles from './wordmark.module.css'

// The product's own name, set rather than drawn: the lockup in
// `docs/brand/` without its mark, since the mark is already in the menu
// bar the panel hangs from. The three colours are the lockup's own.
export function Wordmark() {
  return (
    <span className={styles.root}>
      supolka(<span className={styles.name}>quotos</span>)<span className={styles.bar}>│</span>
    </span>
  )
}
