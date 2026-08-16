import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { LimitWindow } from "./LimitWindow.jsx";

afterEach(cleanup);

// R3-2 regression guard: the captain's crop showed the `Fable` model tag and
// its percent sitting wrong relative to each other and the row. Root cause
// (see RESULT.md) was the scope-tag Badge's caller overriding the base
// Badge's `display: inline-flex` with `inline-block` to get ellipsis
// truncation working — inside a flex row that blockifies either way (a real
// browser turns *either* value's outer display to block for a flex child,
// confirmed live via chrome-devtools, though jsdom's simpler CSS engine
// doesn't model that), so the override did nothing for truncation but did
// kill the badge's own internal `align-items: center`, leaving its text
// sitting at the top of the pill instead of centered.
//
// R5 correction: R3-2's fix (deleting the override) restored centering but
// its "ellipsis works fine on the unmodified inline-flex base" claim was
// wrong — `text-overflow` only applies to block containers, never to a flex
// container, so the badge hard-clipped a long tag mid-character with no "…"
// drawn (verified live via chrome-devtools with an A/B fixture). Truncation
// now lives on an inner block span; the badge keeps inline-flex (centering)
// and only the width constraints. jsdom can't render either behaviour, but
// it can pin the specified styles that make both work in a real browser.
describe("LimitWindow scope badge", () => {
  it("keeps the base Badge's inline-flex display (not inline-block) so its text stays vertically centered", () => {
    const { getByText } = render(
      <LimitWindow name="Weekly" used={45} scope="Fable" resetLabel="Resets Tue at 9:05 PM" />,
    );
    const badge = getByText("Fable").parentElement;
    expect(getComputedStyle(badge).display).toBe("inline-flex");
  });

  it("truncates a long scope tag via an inner block span, where text-overflow actually applies", () => {
    const longName = "Claude Opus 4.5 (extended thinking, research preview)";
    const { getByText } = render(
      <LimitWindow name="Extended thinking" used={61} scope={longName} />,
    );
    const inner = getByText(longName);
    const style = getComputedStyle(inner);
    expect(style.display).toBe("block");
    expect(style.overflow).toBe("hidden");
    expect(style.textOverflow).toBe("ellipsis");
    expect(style.whiteSpace).toBe("nowrap");
    // The badge itself must not reintroduce text-overflow: on a flex
    // container it silently hard-clips instead of ellipsizing.
    const badgeStyle = getComputedStyle(inner.parentElement);
    expect(badgeStyle.display).toBe("inline-flex");
    expect(badgeStyle.textOverflow).not.toBe("ellipsis");
  });

  it("renders with no scope tag at all (the common case) without a badge element", () => {
    const { queryByText } = render(<LimitWindow name="Session" used={78} />);
    expect(queryByText("Fable")).toBeNull();
  });
});
