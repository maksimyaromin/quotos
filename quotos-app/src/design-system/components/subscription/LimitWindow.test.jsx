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
// sitting at the top of the pill instead of centered. Ellipsis works fine on
// the unmodified inline-flex base, so the fix was deleting the override, not
// adding a wrapper. jsdom can't verify the post-blockification result, but it
// can verify the one thing that actually matters here: the *specified*
// display never regresses back to inline-block.
describe("LimitWindow scope badge", () => {
  it("keeps the base Badge's inline-flex display (not inline-block) so its text stays vertically centered", () => {
    const { getByText } = render(
      <LimitWindow name="Weekly" used={45} scope="Fable" resetLabel="Resets Tue at 9:05 PM" />,
    );
    const badge = getByText("Fable");
    expect(getComputedStyle(badge).display).toBe("inline-flex");
  });

  it("still truncates a long scope tag with an ellipsis", () => {
    const longName = "Claude Opus 4.5 (extended thinking, research preview)";
    const { getByText } = render(<LimitWindow name="Extended thinking" used={61} scope={longName} />);
    const badge = getByText(longName);
    const style = getComputedStyle(badge);
    expect(style.display).toBe("inline-flex");
    expect(style.overflow).toBe("hidden");
    expect(style.textOverflow).toBe("ellipsis");
  });

  it("renders with no scope tag at all (the common case) without a badge element", () => {
    const { queryByText } = render(<LimitWindow name="Session" used={78} />);
    expect(queryByText("Fable")).toBeNull();
  });
});
