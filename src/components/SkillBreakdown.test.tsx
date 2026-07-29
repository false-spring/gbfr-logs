import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(cleanup);

vi.mock("@/assets/skill-groups", () => ({ default: {} }));

import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, SkillState } from "@/types";
import { SkillBreakdown } from "./SkillBreakdown";

useMeterSettingsStore.setState({ show_full_values: true });

const baseSkill: SkillState = {
  actionType: { Normal: 1 },
  childCharacterType: "Pl1000",
  hits: 10,
  minDamage: 100,
  maxDamage: 500,
  totalDamage: 3000,
  totalStunValue: 42,
  minStunValue: 2,
  maxStunValue: 12,
  stunHits: 5,
};

const player: ComputedPlayerState = {
  index: 0,
  characterType: "Pl1000",
  totalDamage: 3000,
  dps: 100,
  sba: 0,
  totalStunValue: 42,
  stunPerSecond: 1.4,
  lastDamageTime: 0,
  skillBreakdown: [baseSkill],
  totalDamageTaken: 0,
  healDone: 0,
  healReceived: 0,
  healProvidedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
  healReceivedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
  sbaBreakdown: [],
  totalSbaAdded: 0,
  percentage: 100,
  partyIndex: 0,
};

const neverStaggersSkill: SkillState = {
  actionType: { Normal: 2 },
  childCharacterType: "Pl1000",
  hits: 8,
  minDamage: 50,
  maxDamage: 200,
  totalDamage: 900,
  totalStunValue: 0,
  minStunValue: 0,
  maxStunValue: 0,
  stunHits: 0,
};

const playerWithNoStagger: ComputedPlayerState = {
  ...player,
  totalStunValue: 0,
  stunPerSecond: 0,
  skillBreakdown: [neverStaggersSkill],
};

const damagelessStunSkill: SkillState = {
  actionType: { Normal: 3 },
  childCharacterType: "Pl1000",
  hits: 0,
  minDamage: null,
  maxDamage: null,
  totalDamage: 0,
  totalStunValue: 15,
  minStunValue: 3,
  maxStunValue: 5,
  stunHits: 3,
};

const playerWithDamagelessStunSkill: ComputedPlayerState = {
  ...player,
  totalDamage: 3000,
  totalStunValue: 42 + 15,
  skillBreakdown: [baseSkill, damagelessStunSkill],
};

describe("SkillBreakdown", () => {
  it("renders the full damage column set by default", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={player} color="#fff" />
        </tbody>
      </table>
    );

    expect(screen.queryByText("Hits")).toBeTruthy();
    expect(screen.queryByText("Min")).toBeTruthy();
    expect(screen.queryByText("Max")).toBeTruthy();
    expect(screen.queryByText("Avg")).toBeTruthy();
    expect(screen.getAllByText("%").length).toBeGreaterThan(0);
    expect(screen.queryByText("3,000")).toBeTruthy(); // Total
    expect(screen.queryByText("500")).toBeTruthy(); // Max
    expect(screen.queryByText("300")).toBeTruthy(); // Avg (3000 / 10 hits)

    expect(screen.queryByText("Per Hit")).toBeFalsy();
    expect(screen.queryByText("SPS")).toBeFalsy();
  });

  it("shows Stun / Per Hit / SPS / % for the stun metric, no DMG or DPS", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={player} color="#fff" metric="stun" />
        </tbody>
      </table>
    );

    expect(screen.queryByText("Stun")).toBeTruthy();
    expect(screen.queryByText("Min")).toBeTruthy();
    expect(screen.queryByText("Max")).toBeTruthy();
    expect(screen.queryByText("Per Hit")).toBeTruthy();
    expect(screen.queryByText("SPS")).toBeTruthy();
    expect(screen.getAllByText("%").length).toBeGreaterThan(0);
    expect(screen.queryByText("42")).toBeTruthy(); // Total stun
    expect(screen.queryByText("2")).toBeTruthy(); // Min stun
    expect(screen.queryByText("12")).toBeTruthy(); // Max stun
    expect(screen.queryByText("8.4")).toBeTruthy(); // Per hit: 42 / 5 stun-dealing hits
    expect(screen.queryByText("1.4")).toBeTruthy(); // SPS: skill's share of the player's 1.4 stun/sec
    expect(screen.queryByText("100")).toBeTruthy(); // % — sole skill, 100% of stun

    expect(screen.queryByText("Hits")).toBeFalsy();
    expect(screen.queryByText("Avg")).toBeFalsy();
    expect(screen.queryByText("3,000")).toBeFalsy();
    expect(screen.queryByText("4.2")).toBeFalsy();
  });

  it("shows 0 Per Hit for a skill whose hits never staggered", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={playerWithNoStagger} color="#fff" metric="stun" />
        </tbody>
      </table>
    );

    expect(screen.getAllByText("0").length).toBeGreaterThan(0);
    expect(screen.queryByText("NaN")).toBeFalsy();
    expect(screen.queryByText("Infinity")).toBeFalsy();
  });

  it("hides a skill that never dealt damage from the Damage tab", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={playerWithDamagelessStunSkill} color="#fff" />
        </tbody>
      </table>
    );

    expect(screen.queryByText("3,000")).toBeTruthy(); // baseSkill's Total
    expect(screen.queryByText("15")).toBeFalsy(); // the stun-only skill's stun total
    expect(screen.queryByText("3")).toBeFalsy(); // and its stun-hit count
  });

  it("keeps the % sign and prints a recorded zero Min under 'show full values'", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={{ ...player, skillBreakdown: [{ ...baseSkill, minDamage: 0 }] }} color="#fff" />
        </tbody>
      </table>
    );

    const cells = Array.from(document.querySelectorAll("tr.skill-row td")).map((cell) => cell.textContent);
    expect(cells[3]).toBe("0");
    expect(cells[6]).toBe("100%");
  });

  it("still shows a skill that never dealt damage on the Stun tab", () => {
    render(
      <table>
        <tbody>
          <SkillBreakdown player={playerWithDamagelessStunSkill} color="#fff" metric="stun" />
        </tbody>
      </table>
    );

    expect(screen.queryByText("42")).toBeTruthy(); // baseSkill's Total stun
    expect(screen.queryByText("15")).toBeTruthy(); // damagelessStunSkill's Total stun
  });
});
