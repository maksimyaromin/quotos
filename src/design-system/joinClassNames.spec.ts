import { expect, test } from "vitest";
import { joinClassNames } from "./joinClassNames";

test("joins every truthy class name with a single space", () => {
  expect(joinClassNames("button", "primary", "large")).toBe("button primary large");
});

test("drops false, null, undefined and empty strings", () => {
  expect(joinClassNames("button", false, null, undefined, "", "fullWidth")).toBe(
    "button fullWidth",
  );
});

test("returns an empty string when nothing is truthy", () => {
  expect(joinClassNames(false, null, undefined)).toBe("");
});
