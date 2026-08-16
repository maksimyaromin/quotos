import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

// vi.mock is hoisted, so this static import safely resolves against it.
import { loadTracked, saveTracked, type TrackedAccount } from "./persistence";

const LEGACY_KEY = "quotos.tracked.v1";

const SAMPLE: TrackedAccount = {
  id: "claude:claude",
  provider: "claude",
  config_dir: "~/.claude",
  label: "Renamed Personal",
  pinnedWindowIds: ["weekly_all"],
};

function setNative(): void {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
}

function clearNative(): void {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
}

describe("persistence (browser/mock harness path — no native store)", () => {
  beforeEach(() => {
    clearNative();
    window.localStorage.clear();
    invoke.mockReset();
  });

  it("reads and writes localStorage directly, never invoking the Rust side", async () => {
    await saveTracked([SAMPLE]);
    expect(invoke).not.toHaveBeenCalled();
    const loaded = await loadTracked();
    expect(loaded).toEqual([SAMPLE]);
  });
});

// Followup-3: this is the exact scenario his real upgrade goes through — a
// pre-R2-5 build's localStorage list (custom name + pin) meeting the new
// native-file code for the first time.
describe("persistence (native path — migration, followup-3)", () => {
  beforeEach(() => {
    setNative();
    window.localStorage.clear();
    invoke.mockReset();
  });

  afterEach(() => {
    clearNative();
  });

  it("migrates a legacy localStorage list into the native store on first run", async () => {
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, tracked: [SAMPLE] }));
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "load_tracked") return Promise.resolve([]); // native store starts empty
      if (cmd === "save_tracked") return Promise.resolve();
      throw new Error(`unexpected command ${cmd}`);
    });

    const loaded = await loadTracked();

    expect(loaded).toEqual([SAMPLE]);
    expect(invoke).toHaveBeenCalledWith("save_tracked", { tracked: [SAMPLE] });
  });

  it("never clears or overwrites the legacy localStorage key after migrating", async () => {
    const raw = JSON.stringify({ version: 1, tracked: [SAMPLE] });
    window.localStorage.setItem(LEGACY_KEY, raw);
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.resolve([]) : Promise.resolve(),
    );

    await loadTracked();

    // Followup-3: a bad migration must be recoverable by going back to the
    // old build — that only works if the old key was never touched.
    expect(window.localStorage.getItem(LEGACY_KEY)).toBe(raw);
  });

  it("migrates once, never twice: a non-empty native store is never overwritten from stale localStorage", async () => {
    const nativeAlready: TrackedAccount = { ...SAMPLE, label: "Already Native" };
    window.localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, tracked: [SAMPLE] }));
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.resolve([nativeAlready]) : Promise.resolve(),
    );

    const loaded = await loadTracked();

    expect(loaded).toEqual([nativeAlready]);
    expect(invoke).not.toHaveBeenCalledWith("save_tracked", expect.anything());
  });

  it("stays empty when the legacy key is absent — a fresh install migrates nothing", async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.resolve([]) : Promise.resolve(),
    );

    const loaded = await loadTracked();

    expect(loaded).toEqual([]);
    expect(invoke).not.toHaveBeenCalledWith("save_tracked", expect.anything());
  });

  it("stays empty when the legacy key is corrupt JSON, without throwing", async () => {
    window.localStorage.setItem(LEGACY_KEY, "{ not valid json");
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.resolve([]) : Promise.resolve(),
    );

    const loaded = await loadTracked();

    expect(loaded).toEqual([]);
    expect(invoke).not.toHaveBeenCalledWith("save_tracked", expect.anything());
  });

  it("loadTracked falls back to empty if the native invoke itself fails", async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.reject(new Error("no such command")) : Promise.resolve(),
    );

    const loaded = await loadTracked();

    expect(loaded).toEqual([]);
  });

  it("saveTracked calls the native command with the tracked list", async () => {
    invoke.mockResolvedValue(undefined);
    await saveTracked([SAMPLE]);
    expect(invoke).toHaveBeenCalledWith("save_tracked", { tracked: [SAMPLE] });
  });

  // R2: the caller holds the "what was last saved" record, so a failure
  // swallowed here was a save remembered as done — never retried, and the
  // change died with the process.
  it("saveTracked rejects when the native write fails, so the caller can retry", async () => {
    invoke.mockRejectedValue(new Error("disk full"));
    await expect(saveTracked([SAMPLE])).rejects.toThrow("disk full");
  });

  it("a failed migration save still returns the legacy list and leaves the migration to re-fire next launch", async () => {
    const raw = JSON.stringify({ version: 1, tracked: [SAMPLE] });
    window.localStorage.setItem(LEGACY_KEY, raw);
    invoke.mockImplementation((cmd: string) =>
      cmd === "load_tracked" ? Promise.resolve([]) : Promise.reject(new Error("disk full")),
    );

    const loaded = await loadTracked();

    // This session still shows his real list from memory; the native store
    // stayed empty and the source key untouched, so the next launch simply
    // migrates again.
    expect(loaded).toEqual([SAMPLE]);
    expect(window.localStorage.getItem(LEGACY_KEY)).toBe(raw);
  });
});
