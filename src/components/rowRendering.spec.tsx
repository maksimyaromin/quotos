import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { SubscriptionRow } from "@/design-system";
import { presentRow } from "@/lib/rowPresentation";
import type { Subscription } from "@/types/entities";

const NOW = new Date("2026-08-15T14:00:00Z").getTime();
const IN_AN_HOUR = new Date(NOW + 59 * 60 * 1000).toISOString();

function sub(overrides: Partial<Subscription> = {}): Subscription {
  return {
    id: "claude:claude",
    provider: "claude",
    providerName: "Anthropic",
    label: "Claude Max",
    labelOverride: null,
    account: null,
    state: "working",
    severity: "healthy",
    used: 20,
    resetsAt: null,
    lastReadAt: null,
    windows: [],
    reason: null,
    needsSignIn: false,
    pinnedWindowIds: [],
    headlineWindowId: null,
    configDir: "/Users/someone/.claude",
    rateLimitedUntil: null,
    signInInProgress: false,
    pendingRemoval: false,
    ...overrides,
  };
}

/** Renders a row exactly as App.tsx does, so what is asserted is what the
 * panel actually paints. */
function renderRow(s: Subscription) {
  const presentation = presentRow(s, NOW);
  return render(
    <SubscriptionRow
      label={s.labelOverride ?? s.label}
      provider={s.providerName}
      state={s.state}
      used={s.used}
      severity={s.severity}
      lastRead={undefined}
      windows={[]}
      reason={s.reason ?? undefined}
      badge={presentation.badge}
      actionLabel={presentation.actionLabel}
      footerNote={presentation.footerNote}
      signInInProgress={s.signInInProgress}
    />,
  );
}

/** Counts clock times such as "4:59 PM" anywhere in the rendered row. */
function clockTimesIn(container: HTMLElement): string[] {
  return container.textContent?.match(/\b\d{1,2}:\d{2}\s?(?:AM|PM)?/g) ?? [];
}

describe("subscription row never shows contradictory sign-in and rate-budget states", () => {
  test("a sign-in row shows the sign-in story only, with no rate-budget wait and no clock time", () => {
    const { container } = renderRow(
      sub({
        state: "broken",
        needsSignIn: true,
        used: null,
        reason: "The sign-in expired. Log in again in Claude Code and Quotos will pick it up.",
        rateLimitedUntil: IN_AN_HOUR,
      }),
    );

    expect(screen.getByText("Needs sign-in")).toBeTruthy();
    expect(screen.getByText(/The sign-in expired\./)).toBeTruthy();
    expect(container.textContent).not.toMatch(/rate budget/i);
    expect(container.textContent).not.toMatch(/Retry at/);
    expect(clockTimesIn(container)).toHaveLength(0);
  });

  test("a waiting row states the time exactly once", () => {
    const { container } = renderRow(
      sub({
        state: "behind",
        used: 20,
        rateLimitedUntil: IN_AN_HOUR,
        reason: "The provider didn't answer.",
      }),
    );

    expect(container.textContent).toMatch(/Waiting for the rate budget/);
    expect(clockTimesIn(container)).toHaveLength(1);
    expect(container.textContent).not.toMatch(/Retry at/);
  });

  test("a broken row that is not a sign-in problem never accuses the account of being signed out", () => {
    const { container } = renderRow(
      sub({
        state: "broken",
        needsSignIn: false,
        used: null,
        reason: "This account's access token has expired and Quotos couldn't renew it here.",
      }),
    );

    expect(container.textContent).not.toMatch(/Needs sign-in/);
    expect(container.textContent).toMatch(/couldn't renew it here/);
    expect(screen.getByTitle("Try again")).toBeTruthy();
  });

  test("a healthy row shows its number and nothing alarming", () => {
    const { container } = renderRow(sub());
    expect(container.textContent).toMatch(/20/);
    expect(container.textContent).not.toMatch(/Needs sign-in/);
    expect(container.textContent).not.toMatch(/rate budget/i);
  });
});
