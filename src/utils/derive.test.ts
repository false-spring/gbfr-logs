import { ComputedPlayerState, EncounterState, EnemyState, MeterColumns, PlayerState, SkillState } from "@/types";
import { describe, expect, it } from "vitest";
import { buildEnemyGroups, combineEncounterStates, isAttributedCause, skillRowKey, sortPlayers } from "./derive";

const enemy = (index: number, typeHash: number, totalDamage: number): EnemyState => ({
  index,
  targetType: { Unknown: typeHash },
  baseTargetType: { Unknown: typeHash },
  totalDamage,
});

const EM7220 = 0x67cca534;
const EM7221 = 0xfbc2c2a3;
const UNGROUPED = 0xdeadbeef;

describe("buildEnemyGroups", () => {
  it("folds a merge group's members into one entry with summed damage", () => {
    const groups = buildEnemyGroups([enemy(12, UNGROUPED, 500), enemy(10, EM7220, 300), enemy(11, EM7221, 250)]);

    expect(groups).toHaveLength(2);

    const twins = groups[0];
    expect(twins.totalDamage).toBe(550);
    expect(twins.indices).toEqual([10, 11]);
    expect(twins.representativeIndex).toBe(10);

    const lone = groups[1];
    expect(lone.totalDamage).toBe(500);
    expect(lone.indices).toEqual([12]);
    expect(lone.representativeIndex).toBe(12);
  });

  it("picks the same representative whatever order the members arrive in", () => {
    const inIndexOrder = buildEnemyGroups([enemy(1, UNGROUPED, 50), enemy(3, EM7220, 100), enemy(5, EM7221, 900)]);
    const inDamageOrder = buildEnemyGroups([enemy(5, EM7221, 900), enemy(3, EM7220, 100), enemy(1, UNGROUPED, 50)]);

    expect(inIndexOrder.map((group) => group.representativeIndex)).toEqual(
      inDamageOrder.map((group) => group.representativeIndex)
    );
    expect(inIndexOrder[0].representativeIndex).toBe(5);
    expect(inIndexOrder[0].totalDamage).toBe(1000);
  });

  it("leaves an ungrouped enemy as its own single-member entry", () => {
    const groups = buildEnemyGroups([enemy(7, UNGROUPED, 999)]);

    expect(groups).toHaveLength(1);
    expect(groups[0].indices).toEqual([7]);
    expect(groups[0].representativeIndex).toBe(7);
    expect(groups[0].totalDamage).toBe(999);
  });
});

const skill = (overrides: Partial<SkillState>): SkillState => ({
  actionType: { Normal: 1 },
  childCharacterType: "Pl1000",
  hits: 0,
  minDamage: null,
  maxDamage: null,
  totalDamage: 0,
  totalStunValue: 0,
  minStunValue: null,
  maxStunValue: null,
  stunHits: 0,
  ...overrides,
});

const player = (overrides: Partial<PlayerState>): PlayerState => ({
  index: 0,
  characterType: "Pl1000",
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
  sbaBreakdown: [],
  totalSbaAdded: 0,
  ...overrides,
});

const encounterState = (overrides: Partial<EncounterState>): EncounterState => ({
  totalDamage: 0,
  dps: 0,
  startTime: 0,
  endTime: 0,
  party: {},
  status: "Stopped",
  targets: {},
  ...overrides,
});

describe("skillRowKey", () => {
  it("separates auras that share the 99999 sentinel", () => {
    const sentinel = { actionType: { Normal: 99999 } as const, childCharacterType: "Pl2600" as const };

    const keys = (["ToxicBlast", "LusterOfDarkness", "MysteryBox", "IceAndFireFollowUp"] as const).map((auraSource) =>
      skillRowKey({ ...sentinel, auraSource })
    );

    expect(new Set(keys).size).toBe(keys.length);
  });

  it("leaves non-aura rows unchanged", () => {
    const row = { actionType: { Normal: 150 } as const, childCharacterType: "Pl2600" as const };

    expect(skillRowKey(row)).toBe("Pl2600-Normal-150");
    expect(skillRowKey({ ...row, auraSource: "None" })).toBe("Pl2600-Normal-150");
  });
});

describe("combineEncounterStates", () => {
  it("sums member states over the combined window without mutating the sources", () => {
    const stateA = encounterState({
      totalDamage: 300,
      startTime: 1000,
      endTime: 4000,
      party: {
        0: player({
          index: 0,
          totalDamage: 200,
          sba: 100,
          totalStunValue: 20,
          lastDamageTime: 3000,
          skillBreakdown: [
            skill({
              hits: 2,
              minDamage: 50,
              maxDamage: 150,
              totalDamage: 200,
              totalStunValue: 20,
              minStunValue: 5,
              maxStunValue: 15,
              stunHits: 2,
            }),
          ],
        }),
        1: player({
          index: 1,
          characterType: "Pl1100",
          totalDamage: 100,
          skillBreakdown: [
            skill({
              actionType: { Normal: 5 },
              childCharacterType: "Pl1100",
              hits: 1,
              minDamage: 100,
              maxDamage: 100,
              totalDamage: 100,
            }),
          ],
        }),
      },
      targets: { 10: enemy(10, EM7220, 300) },
    });
    const stateB = encounterState({
      totalDamage: 150,
      startTime: 1500,
      endTime: 5000,
      party: {
        0: player({
          index: 0,
          totalDamage: 150,
          sba: 100,
          totalStunValue: 10,
          lastDamageTime: 4500,
          skillBreakdown: [
            skill({
              hits: 1,
              minDamage: 40,
              maxDamage: 120,
              totalDamage: 150,
              totalStunValue: 10,
              minStunValue: 3,
              maxStunValue: 12,
              stunHits: 1,
            }),
          ],
        }),
      },
      targets: { 11: enemy(11, EM7221, 150) },
    });

    const combined = combineEncounterStates([stateA, stateB]);

    expect(combined.startTime).toBe(1000);
    expect(combined.endTime).toBe(5000);
    expect(combined.totalDamage).toBe(450);
    expect(combined.dps).toBeCloseTo(112.5);

    const p0 = combined.party[0];
    expect(p0.totalDamage).toBe(350);
    expect(p0.totalStunValue).toBe(30);
    expect(p0.dps).toBeCloseTo(87.5);
    expect(p0.stunPerSecond).toBeCloseTo(7.5);
    expect(p0.skillBreakdown).toHaveLength(1);
    expect(p0.skillBreakdown[0]).toMatchObject({
      hits: 3,
      totalDamage: 350,
      totalStunValue: 30,
      stunHits: 3,
      minDamage: 40,
      maxDamage: 150,
      minStunValue: 3,
      maxStunValue: 15,
    });

    expect(combined.party[1].totalDamage).toBe(100);
    expect(combined.party[1].dps).toBeCloseTo(25);

    expect(Object.keys(combined.targets)).toHaveLength(2);

    expect(stateA.party[0].totalDamage).toBe(200);
    expect(stateA.party[0].skillBreakdown[0].hits).toBe(2);
  });
});

describe("sortPlayers", () => {
  const player = (index: number, totalSbaAdded: number, sba: number) =>
    ({ index, partyIndex: index, totalSbaAdded, sba }) as ComputedPlayerState;

  it("sorts by gauge generated, not by current gauge level", () => {
    const players = [player(0, 100, 900), player(1, 300, 100), player(2, 200, 500)];

    sortPlayers(players, MeterColumns.TotalSbaAdded, "desc");
    expect(players.map((p) => p.index)).toEqual([1, 2, 0]);

    sortPlayers(players, MeterColumns.TotalSbaAdded, "asc");
    expect(players.map((p) => p.index)).toEqual([0, 2, 1]);
  });

  it("ranks by percentage the same way", () => {
    const players = [player(0, 100, 0), player(1, 300, 0), player(2, 200, 0)];

    sortPlayers(players, MeterColumns.SbaPercentage, "desc");
    expect(players.map((p) => p.index)).toEqual([1, 2, 0]);
  });

  it("still sorts by the current gauge level under the SBA column", () => {
    const players = [player(0, 100, 900), player(1, 300, 100), player(2, 200, 500)];

    sortPlayers(players, MeterColumns.SBA, "desc");
    expect(players.map((p) => p.index)).toEqual([0, 2, 1]);
  });

  it("treats a missing generated total as zero rather than reordering on NaN", () => {
    const players = [{ index: 0, partyIndex: 0, sba: 0 } as ComputedPlayerState, player(1, 50, 0)];

    sortPlayers(players, MeterColumns.TotalSbaAdded, "desc");
    expect(players.map((p) => p.index)).toEqual([1, 0]);
  });
});

describe("isAttributedCause", () => {
  it("counts a party-wide fill as explained", () => {
    expect(isAttributedCause("InferredPartyFill")).toBe(true);
  });

  it("still counts an unread peer sync and an unnameable gain as unexplained", () => {
    expect(isAttributedCause("Remote")).toBe(false);
    expect(isAttributedCause("Unknown")).toBe(false);
    expect(isAttributedCause("NotClassified")).toBe(false);
  });
});
