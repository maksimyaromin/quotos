import * as React from "react";

export interface TextFieldProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "style"> {
  /** Label shown above the field. */
  label?: string;
  value?: string;
  placeholder?: string;
  /** Render the input value in IBM Plex Mono — used for pasted keys/tokens. */
  mono?: boolean;
  /** Red border for a failed field (e.g. verification failed). */
  invalid?: boolean;
  disabled?: boolean;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  style?: React.CSSProperties;
}

/** macOS small text input for the add-subscription flow (paste a key, name an account). */
export function TextField(props: TextFieldProps): React.ReactElement;
