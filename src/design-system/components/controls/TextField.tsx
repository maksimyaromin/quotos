import * as React from "react";

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
  ...rest
}: TextFieldProps) {
  const [focus, setFocus] = React.useState(false);
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)", ...style }}>
      {label ? (
        <span
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "var(--text-sm)",
            color: "var(--text-secondary)",
          }}
        >
          {label}
        </span>
      ) : null}
      <input
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={onChange}
        onFocus={() => setFocus(true)}
        onBlur={() => setFocus(false)}
        style={{
          height: "var(--control-height-lg)",
          padding: "0 var(--space-2)",
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
          fontSize: "var(--text-base)",
          color: "var(--text-primary)",
          background: "var(--bg-input)",
          border: `0.5px solid ${invalid ? "var(--red)" : focus ? "var(--border-focus)" : "var(--border-default)"}`,
          borderRadius: "var(--radius-sm)",
          outline: "none",
          boxShadow: focus ? "var(--shadow-focus)" : "none",
          transition: "border-color var(--dur-fast), box-shadow var(--dur-fast)",
        }}
        {...rest}
      />
    </label>
  );
}
