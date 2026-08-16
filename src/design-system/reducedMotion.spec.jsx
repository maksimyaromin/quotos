import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CapacityBar } from "./components/indicators/CapacityBar.jsx";

afterEach(cleanup);

// macOS "Reduce Motion" reaches the webview as prefers-reduced-motion, and
// tokens/elevation.css answers it in one place: zero the --dur-* tokens
// (which every transition in the app takes its duration from), stop the
// inline-styled keyframe animations, and hide the shimmer overlay (stopped,
// its gradient would sit as a static white stripe). jsdom applies none of
// this, so these tests pin the pieces the media query relies on instead:
// the block itself, its token coverage, the no-literal-durations invariant
// that makes token-zeroing complete, and the shimmer's targeting hook.

const dsDir = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(dsDir, "..");
const elevation = readFileSync(path.join(dsDir, "tokens/elevation.css"), "utf8");
const reducedBlock = elevation.split("@media (prefers-reduced-motion: reduce)")[1];

function sourceFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...sourceFiles(full));
    else if (/\.(jsx|tsx|ts|css)$/.test(entry.name) && !/\.test\./.test(entry.name))
      files.push(full);
  }
  return files;
}

describe("prefers-reduced-motion contract", () => {
  it("elevation.css has the reduced-motion block", () => {
    expect(reducedBlock).toBeTruthy();
  });

  it("zeroes every --dur-* token the base block defines", () => {
    const tokens = new Set(elevation.match(/--dur-[a-z]+(?=\s*:)/g));
    expect(tokens.size).toBeGreaterThanOrEqual(4);
    for (const token of tokens) {
      expect(reducedBlock).toContain(`${token}: 0ms`);
    }
  });

  it("stops keyframe animations and hides the shimmer overlay", () => {
    // The keyframe animations carry literal durations in inline styles, so
    // only !important reaches them.
    expect(reducedBlock).toMatch(/animation:\s*none\s*!important/);
    expect(reducedBlock).toContain("[data-quotos-shimmer]");
  });

  it("every transition in app source takes its duration from a --dur token", () => {
    // This is what makes zeroing the tokens complete coverage: a transition
    // with a literal duration would keep moving under Reduce Motion.
    for (const file of sourceFiles(srcDir)) {
      for (const line of readFileSync(file, "utf8").split("\n")) {
        if (/transition\s*:/.test(line)) {
          expect(line, `${file} declares a transition without a --dur token`).toContain(
            "var(--dur",
          );
        }
      }
    }
  });

  it("CapacityBar's reading shimmer carries the data-quotos-shimmer hook", () => {
    const reading = render(<CapacityBar used={40} reading />);
    expect(reading.container.querySelector("[data-quotos-shimmer]")).not.toBeNull();
    cleanup();
    const settled = render(<CapacityBar used={40} />);
    expect(settled.container.querySelector("[data-quotos-shimmer]")).toBeNull();
  });
});
