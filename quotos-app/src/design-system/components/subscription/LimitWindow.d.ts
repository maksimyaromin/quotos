import * as React from "react";

export interface LimitWindowProps {
  /** Stable id for this window (see `LimitWindowEntity.id`) — passed back to `onTogglePin`. */
  id?: string;
  /** The provider's own wording for this window — rendered verbatim, truncated if long. */
  name: string;
  /** Percent consumed 0–100, or null when the provider reports no percentage. */
  used?: number | null;
  /** Exact reset copy, e.g. "Resets today at 4:05 PM" — or null. */
  resetLabel?: string | null;
  /** Optional scope tag when the window applies to part of the subscription (e.g. "Opus 4"). */
  scope?: string | null;
  /** Dim when the parent subscription is behind. */
  stale?: boolean;
  /** v4: whether this window's own figure is in the menu bar. Always
   * renders its pin button (leading the row) regardless of this value. */
  pinned?: boolean;
  /** Called with this window's `id` when its pin button is clicked. */
  onTogglePin?: (id: string) => void;
  style?: React.CSSProperties;
}

/** One row in a subscription's variable detail list. Handles missing percentage / reset gracefully. */
export function LimitWindow(props: LimitWindowProps): React.ReactElement;
