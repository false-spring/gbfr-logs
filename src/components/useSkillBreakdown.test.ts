import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

// skill-groups.ts reads its JSON through the Tauri fs/path bridge, absent under
// jsdom. Stubbed with Pl2800's real grouping: `dead-lands` claims 5000 and 5010,
// the two ids the ether gun also reports.
vi.mock("@/assets/skill-groups", () => ({
  default: { Pl2800: { "dead-lands": { skills: [5000, 5010] } } },
}));

vi.mock("i18next", () => ({
  t: (keys: string | string[]) => (Array.isArray(keys) ? keys[0] : keys),
}));

import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, SkillState } from "@/types";

import { useSkillBreakdown } from "./useSkillBreakdown";

const skill = (over: Partial<SkillState>): SkillState => ({
  actionType: { Normal: 5000 },
  childCharacterType: "Pl2800",
  hits: 1,
  minDamage: 100,
  maxDamage: 100,
  totalDamage: 100,
  totalStunValue: 0,
  minStunValue: 0,
  maxStunValue: 0,
  stunHits: 0,
  ...over,
});

const player = (skillBreakdown: SkillState[]): ComputedPlayerState => ({
  index: 0,
  characterType: "Pl2800",
  totalDamage: skillBreakdown.reduce((acc, s) => acc + s.totalDamage, 0),
  dps: 0,
  sba: 0,
  totalStunValue: 0,
  stunPerSecond: 0,
  lastDamageTime: 0,
  skillBreakdown,
  totalDamageTaken: 0,
  healDone: 0,
  healReceived: 0,
  healProvidedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
  healReceivedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
  sbaBreakdown: [],
  totalSbaAdded: 0,
  percentage: 100,
  partyIndex: 0,
});

const isGroup = (row: { actionType: unknown }) =>
  typeof row.actionType === "object" && row.actionType !== null && Object.hasOwn(row.actionType, "Group");

describe("useSkillBreakdown, condensing the ether gun", () => {
  it("keeps the gun out of the group that claims its ids", () => {
    useMeterSettingsStore.setState({ use_condensed_skills: true });

    const deadLands = skill({ actionType: { Normal: 5000 }, totalDamage: 900 });
    const gun = skill({ actionType: { Normal: 5000 }, totalDamage: 100, etherGun: true });

    const { result } = renderHook(() => useSkillBreakdown(player([deadLands, gun])));

    expect(result.current.skills).toHaveLength(2);

    const group = result.current.skills.find(isGroup);
    expect(group?.totalDamage).toBe(900);

    const loose = result.current.skills.find((row) => !isGroup(row));
    expect(loose?.totalDamage).toBe(100);
    expect((loose as SkillState).etherGun).toBe(true);
  });

  it("still condenses the skill's own hits at both ids", () => {
    useMeterSettingsStore.setState({ use_condensed_skills: true });

    const multihit = skill({ actionType: { Normal: 5000 }, totalDamage: 900 });
    const finisher = skill({ actionType: { Normal: 5010 }, totalDamage: 300 });

    const { result } = renderHook(() => useSkillBreakdown(player([multihit, finisher])));

    expect(result.current.skills).toHaveLength(1);
    expect(result.current.skills[0].totalDamage).toBe(1200);
    expect(isGroup(result.current.skills[0])).toBe(true);
  });

  it("leaves both rows alone when condensing is off", () => {
    useMeterSettingsStore.setState({ use_condensed_skills: false });

    const deadLands = skill({ actionType: { Normal: 5000 }, totalDamage: 900 });
    const gun = skill({ actionType: { Normal: 5000 }, totalDamage: 100, etherGun: true });

    const { result } = renderHook(() => useSkillBreakdown(player([deadLands, gun])));

    expect(result.current.skills).toHaveLength(2);
    expect(result.current.skills.some(isGroup)).toBe(false);
  });
});
