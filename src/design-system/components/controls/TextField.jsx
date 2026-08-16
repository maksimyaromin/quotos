import React from "react";

/** A labeled text input in macOS small-control style. Used in the
 *  add-subscription flow (paste a key, name an account). */
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
}) {
  const [focus, setFocus] = React.useState(false);
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)", ...style }}>
      {label ? (
        <span style={{
          fontFamily: "var(--font-sans)",
          fontSize: "var(--text-sm)",
          color: "var(--text-secondary)",
        }}>{label}</span>
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
