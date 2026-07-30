import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(cleanup);
vi.mock("@/assets/skill-groups", () => ({
  default: {
    Pl1000: { "normal-attack": { skills: [100, 110, 120] } },
    Pl2000: { "normal-attack": { skills: [100, 110, 120] } },
  },
}));

// The label table repeats in each factory: vi.mock is hoisted above top-level
// bindings, so referencing a shared const is a TDZ error at collection time.
vi.mock("react-i18next", () => {
  const EN: Record<string, string> = {
    "ui.sba-breakdown.Remote": "Other players (not reported)",
    "ui.sba-breakdown.DamageTaken": "Damage Taken",
    "ui.sba-breakdown.InferredChainGrant": "SBA Chain Bonus",
    "ui.sba-breakdown.ChainGrant": "SBA Chain Bonus",
    "ui.sba-breakdown.Unknown": "Unattributed",
    "ui.logs.sba-source": "Source",
  };
  return {
    useTranslation: () => ({
      t: (key: string, opts?: Record<string, unknown>) =>
        opts?.percentage !== undefined ? `${opts.percentage}%` : EN[key] ?? key,
    }),
  };
});

vi.mock("i18next", () => {
  const EN: Record<string, string> = {
    "ui.sba-breakdown.Remote": "Other players (not reported)",
    "ui.sba-breakdown.DamageTaken": "Damage Taken",
    "ui.sba-breakdown.ChainGrant": "SBA Chain Bonus",
    "ui.sba-breakdown.InferredChainGrant": "SBA Chain Bonus",
    "ui.sba-breakdown.Unknown": "Unattributed",
  };
  const t = (key: string | string[]) => {
    const first = Array.isArray(key) ? key[0] : key;
    return EN[first] ?? first;
  };
  return { t, default: { t } };
});

import { ComputedPlayerState, SbaSourceState } from "@/types";
import { SbaBreakdown } from "./SbaBreakdown";

const source = (overrides: Partial<SbaSourceState>): SbaSourceState => ({
  cause: { Action: { Normal: 610 } },
  childCharacterType: null,
  ticks: 1,
  totalSbaAdded: 100,
  ...overrides,
});

const player = (sbaBreakdown: SbaSourceState[], characterType = "Pl1000"): ComputedPlayerState =>
  ({
    index: 0,
    characterType,
    totalDamage: 0,
    dps: 0,
    sba: 0,
    totalStunValue: 0,
    stunPerSecond: 0,
    lastDamageTime: 0,
    skillBreakdown: [],
    totalDamageTaken: 0,
    healDone: 0,
    healReceived: 0,
    healProvidedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
    healReceivedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
    sbaBreakdown,
    totalSbaAdded: sbaBreakdown.reduce((a, s) => a + s.totalSbaAdded, 0),
    percentage: 100,
    partyIndex: 0,
  }) as ComputedPlayerState;

const renderFor = (sources: SbaSourceState[], characterType?: string) =>
  render(
    <table>
      <tbody>
        <SbaBreakdown player={player(sources, characterType)} color="#fff" />
      </tbody>
    </table>
  );

describe("SbaBreakdown", () => {
  it("shows gauge-shaped columns, not damage ones", () => {
    renderFor([source({})]);

    expect(screen.getByText("Ticks")).toBeDefined();
    expect(screen.getByText("Gen")).toBeDefined();
    expect(screen.getByText("Per Tick")).toBeDefined();
    expect(screen.queryByText("Hits")).toBeNull();
    expect(screen.queryByText("Min")).toBeNull();
    expect(screen.queryByText("Max")).toBeNull();
    expect(screen.queryByText("Avg")).toBeNull();
  });

  it("keeps unattributed rows and counts them in the denominator", () => {
    renderFor([
      source({ cause: { Action: { Normal: 610 } }, totalSbaAdded: 600 }),
      source({ cause: "Remote", totalSbaAdded: 400 }),
    ]);

    expect(screen.getByText("60")).toBeDefined();
    expect(screen.getByText("40")).toBeDefined();
    expect(screen.queryByText("100")).toBeNull();
  });

  it("labels each non-action cause distinctly", () => {
    renderFor([
      source({ cause: "Remote", totalSbaAdded: 100 }),
      source({ cause: "DamageTaken", totalSbaAdded: 100 }),
      source({ cause: "Unknown", totalSbaAdded: 100 }),
    ]);

    expect(screen.getByText(/Other players/)).toBeDefined();
    expect(screen.getByText(/Damage Taken/)).toBeDefined();
    expect(screen.getByText(/Unattributed/)).toBeDefined();
  });

  it("renders an inferred cause under the move's own name", () => {
    renderFor([source({ cause: { Inferred: { Normal: 610 } }, totalSbaAdded: 100 })]);

    expect(screen.queryByText(/inferred/i)).toBeNull();
    expect(screen.getByText(/610|Perfect Dodge/)).toBeDefined();
  });

  it("folds a read and an inferred cause into one row", () => {
    const { container } = renderFor([
      source({ cause: { Action: { Normal: 610 } }, ticks: 2, totalSbaAdded: 300 }),
      source({ cause: { Inferred: { Normal: 610 } }, ticks: 3, totalSbaAdded: 700 }),
    ]);

    expect(container.querySelectorAll("tr.skill-row")).toHaveLength(1);
    expect(container.textContent).toContain("5");
    expect(screen.getByText("100")).toBeDefined();
  });

  it("folds a read and an inferred chain grant into one row", () => {
    const { container } = renderFor([
      source({ cause: "ChainGrant", ticks: 1, totalSbaAdded: 100 }),
      source({ cause: "InferredChainGrant", ticks: 1, totalSbaAdded: 100 }),
    ]);

    expect(container.querySelectorAll("tr.skill-row")).toHaveLength(1);
  });

  it("sorts by contribution, descending", () => {
    const { container } = renderFor([
      source({ cause: "DamageTaken", totalSbaAdded: 100 }),
      source({ cause: { Action: { Normal: 610 } }, totalSbaAdded: 900 }),
    ]);

    const firstRow = container.querySelectorAll("tr.skill-row")[0];
    expect(firstRow?.textContent).toContain("90");
  });

  it("names a child actor's move under that actor, not the player", () => {
    const { container } = renderFor(
      [
        source({ cause: { Action: { Normal: 200 } }, childCharacterType: null, totalSbaAdded: 100 }),
        source({ cause: { Action: { Normal: 200 } }, childCharacterType: "Pl2000", totalSbaAdded: 200 }),
      ],
      "Pl1900"
    );

    const rows = [...container.querySelectorAll("tr.skill-row")].map((r) => r.textContent);
    expect(rows).toHaveLength(2);
    expect(rows.some((r) => r?.includes("skills.Pl2000.200"))).toBe(true);
    expect(rows.some((r) => r?.includes("skills.Pl1900.200"))).toBe(true);
  });

  it("condenses a combo into one group row, summing its gauge and ticks", () => {
    const { container } = renderFor([
      source({ cause: { Action: { Normal: 100 } }, ticks: 3, totalSbaAdded: 300 }),
      source({ cause: { Action: { Normal: 110 } }, ticks: 2, totalSbaAdded: 200 }),
      source({ cause: { Action: { Normal: 120 } }, ticks: 1, totalSbaAdded: 100 }),
      source({ cause: "DamageTaken", ticks: 4, totalSbaAdded: 400 }),
    ]);

    const rows = [...container.querySelectorAll("tr.skill-row")];
    expect(rows).toHaveLength(2);

    const group = rows.find((r) => r.className.includes("group"));
    expect(group?.textContent).toContain("skills.Pl1000.skill-groups.normal-attack");
    expect(group?.textContent).toContain("6");
    expect(group?.textContent).toContain("60.0");
  });

  it("expands a group to its members, and collapses again", () => {
    const { container } = renderFor([
      source({ cause: { Action: { Normal: 100 } }, totalSbaAdded: 600 }),
      source({ cause: { Action: { Normal: 110 } }, totalSbaAdded: 400 }),
    ]);

    expect(container.querySelectorAll("tr.skill-row")).toHaveLength(1);

    const group = container.querySelector("tr.skill-row.group") as HTMLElement;
    fireEvent.click(group);

    const nested = [...container.querySelectorAll("tr.skill-row.nested")];
    expect(nested).toHaveLength(2);
    expect(nested[0]?.textContent).toContain("skills.Pl1000.100");
    expect(nested[1]?.textContent).toContain("skills.Pl1000.110");

    fireEvent.click(group);
    expect(container.querySelectorAll("tr.skill-row.nested")).toHaveLength(0);
  });

  it("groups a child actor's combo under its own heading", () => {
    const { container } = renderFor(
      [
        source({ cause: { Action: { Normal: 100 } }, childCharacterType: null, totalSbaAdded: 300 }),
        source({ cause: { Action: { Normal: 110 } }, childCharacterType: null, totalSbaAdded: 200 }),
        source({ cause: { Action: { Normal: 100 } }, childCharacterType: "Pl2000", totalSbaAdded: 500 }),
      ],
      "Pl1000"
    );

    const groups = [...container.querySelectorAll("tr.skill-row.group")].map((r) => r.textContent);
    expect(groups).toHaveLength(2);
    expect(groups.some((g) => g?.includes("skills.Pl2000.skill-groups.normal-attack"))).toBe(true);
    expect(groups.some((g) => g?.includes("skills.Pl1000.skill-groups.normal-attack"))).toBe(true);
  });

  it("never groups a cause that names no move", () => {
    const { container } = renderFor([
      source({ cause: "DamageTaken", totalSbaAdded: 300 }),
      source({ cause: "InferredChainGrant", totalSbaAdded: 300 }),
      source({ cause: "Remote", totalSbaAdded: 400 }),
    ]);

    expect(container.querySelectorAll("tr.skill-row.group")).toHaveLength(0);
    expect(container.querySelectorAll("tr.skill-row")).toHaveLength(3);
  });

  it("renders no NaN for a player who generated nothing", () => {
    const { container } = renderFor([]);

    expect(container.textContent).not.toContain("NaN");
    expect(container.textContent).not.toContain("Infinity");
  });
});
