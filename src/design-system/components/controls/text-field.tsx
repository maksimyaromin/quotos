import type * as React from "react";
import { joinClassNames } from "../../joinClassNames";
import styles from "./text-field.module.css";

export interface TextFieldProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "style"> {
  /** Label shown above the field. */
  label?: string;
  value?: string;
  placeholder?: string;
  /** Renders the input value in MonoLisa, used for pasted keys or tokens. */
  mono?: boolean;
  /** Red border for a failed field, such as a failed verification. */
  invalid?: boolean;
  disabled?: boolean;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  style?: React.CSSProperties;
}

/** macOS small text input for the add-subscription flow, used to paste a key or name an account. */
export function TextField({
  label,
  value,
  placeholder,
  mono = false,
  invalid = false,
  disabled = false,
  onChange,
  style,
  className,
  ...rest
}: TextFieldProps) {
  return (
    <label className={styles.label} style={style}>
      {label ? <span className={styles.labelText}>{label}</span> : null}
      <input
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={onChange}
        data-mono={mono ? "true" : undefined}
        data-invalid={invalid ? "true" : undefined}
        className={joinClassNames(styles.input, className)}
        {...rest}
      />
    </label>
  );
}
