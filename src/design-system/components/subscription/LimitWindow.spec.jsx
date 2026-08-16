import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { LimitWindow } from "./LimitWindow.jsx";

afterEach(cleanup);

// text-overflow only applies to block containers, never a flex container,
// so the scope badge must stay inline-flex for vertical centering while an
// inner block span, not the badge itself, owns the ellipsis truncation.
// jsdom cannot render either behavior, but it can pin the styles that make
// both work in a real browser.
describe("LimitWindow scope badge", () => {
  it("keeps the base Badge's inline-flex display so its text stays vertically centered", () => {
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

  it("renders with no scope tag at all, the common case, without a badge element", () => {
    const { queryByText } = render(<LimitWindow name="Session" used={78} />);
    expect(queryByText("Fable")).toBeNull();
  });
});
