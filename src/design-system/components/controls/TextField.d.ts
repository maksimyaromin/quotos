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
export function TextField(props: TextFieldProps): React.ReactElement;
