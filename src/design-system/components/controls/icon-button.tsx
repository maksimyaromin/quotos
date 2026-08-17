import type * as React from 'react'
import { joinClassNames } from '../../join-class-names'
import styles from './icon-button.module.css'

export interface IconButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  size?: number
  active?: boolean
  disabled?: boolean
  label: string
  onClick?: React.MouseEventHandler<HTMLButtonElement>
  children?: React.ReactNode
}

export function IconButton({
  size = 24,
  active = false,
  disabled = false,
  label,
  onClick,
  children,
  className,
  style,
  ...rest
}: IconButtonProps) {
  const glyphSize = Math.round(size * 0.6)
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      data-active={active ? 'true' : undefined}
      className={joinClassNames(styles.button, className)}
      style={{ width: size, height: size, ...style }}
      {...rest}
    >
      <span className={styles.glyph} style={{ width: glyphSize, height: glyphSize }}>
        {children}
      </span>
    </button>
  )
}
