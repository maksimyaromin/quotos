import type * as React from "react";
import { joinClassNames } from "../../join-class-names";
import styles from "./text-field.module.css";

export interface TextFieldProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "style"> {
  label?: string;
  value?: string;
  placeholder?: string;
  mono?: boolean;
  invalid?: boolean;
  disabled?: boolean;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  style?: React.CSSProperties;
}

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
