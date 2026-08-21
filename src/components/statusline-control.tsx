import { useEffect, useState } from 'react'
import { Button } from '@/design-system'
import { statuslineDisable, statuslineEnable, statuslineStatus } from '@/lib/tauri-client'
import { isStatuslineError, type StatuslineIntegrationStatus } from '@/types/entities'
import styles from './statusline-control.module.css'

function describeError(err: unknown): string {
  if (isStatuslineError(err) && 'message' in err) {
    return err.message
  }
  return "Quotos couldn't do that. Nothing was changed."
}

export function StatuslineControl({ configDir }: { configDir: string }) {
  const [status, setStatus] = useState<StatuslineIntegrationStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void statuslineStatus(configDir)
      .then((s) => {
        if (!cancelled) setStatus(s)
      })
      .catch(() => {
        if (!cancelled) setStatus({ kind: 'not_installed' })
      })
    return () => {
      cancelled = true
    }
  }, [configDir])

  const enable = async () => {
    setBusy(true)
    setError(null)
    try {
      await statuslineEnable(configDir)
      setStatus({ kind: 'installed' })
    } catch (err) {
      setError(describeError(err))
    } finally {
      setBusy(false)
    }
  }

  const disable = async () => {
    setBusy(true)
    setError(null)
    try {
      await statuslineDisable(configDir)
      setStatus({ kind: 'not_installed' })
    } catch (err) {
      setError(describeError(err))
    } finally {
      setBusy(false)
    }
  }

  if (!status) return null

  const installed = status.kind === 'installed'
  const settingsPath = `${configDir}/settings.json`

  return (
    <div className={styles.column}>
      <div className={styles.row}>
        <Button
          size="sm"
          variant="secondary"
          style={installed ? { color: 'var(--red)' } : undefined}
          disabled={busy}
          onClick={() => void (installed ? disable() : enable())}
        >
          {installed ? 'Turn off live updates' : 'Enable live updates'}
        </Button>
      </div>
      <span className={styles.note}>
        {installed
          ? `Live updates are on. Claude Code runs a script Quotos wrote, and Claude Code hides its footer keyboard hints while any status line is configured.`
          : `Enabling writes a status line command into ${settingsPath}, keeping a backup of what was there. It wraps an existing status line if you already have one, and Claude Code hides its footer keyboard hints while any status line is configured.`}
      </span>
      {error ? (
        <span className={styles.note} data-tone="error">
          {error}
        </span>
      ) : null}
    </div>
  )
}
