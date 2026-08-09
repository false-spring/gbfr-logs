import { beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("react-hot-toast", () => ({ default: { success: vi.fn(), error: vi.fn() } }));

import { EncounterState, MeterColumns, PlayerData, PlayerState } from "@/types";

import {
  exportCharacterDataToClipboard,
  exportFullEncounterToClipboard,
  exportSimpleEncounterToClipboard,
} from "./export";

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

const partyMember = (overrides: Partial<PlayerData>): PlayerData => ({
  actorIndex: 0,
  displayName: "",
  characterName: "",
  networkUserId: null,
  networkUserName: null,
  characterType: "Pl1000",
  sigils: [],
  isOnline: true,
  weaponInfo: null,
  overmasteryInfo: null,
  summonInfo: null,
  playerStats: null,
  ...overrides,
});

describe("clipboard exports", () => {
  const written: string[] = [];

  beforeAll(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          written.push(text);
          return Promise.resolve();
        },
      },
    });
  });

  const lastWrite = () => written[written.length - 1];

  const party = [partyMember({ actorIndex: 0, displayName: "Lewd" })];
  const state = encounterState({
    totalDamage: 100,
    dps: 25,
    startTime: 0,
    endTime: 4000,
    party: { 0: player({ index: 0, totalDamage: 100, dps: 25 }) },
  });

  it("omits display names from the encounter exports when names are hidden", () => {
    exportSimpleEncounterToClipboard(MeterColumns.TotalDamage, "desc", state, party, true);
    expect(lastWrite()).toContain("Lewd");

    exportSimpleEncounterToClipboard(MeterColumns.TotalDamage, "desc", state, party, false);
    expect(lastWrite()).not.toContain("Lewd");

    exportFullEncounterToClipboard(MeterColumns.TotalDamage, "desc", state, party, true);
    expect(lastWrite()).toContain("Lewd");

    exportFullEncounterToClipboard(MeterColumns.TotalDamage, "desc", state, party, false);
    expect(lastWrite()).not.toContain("Lewd");
  });

  it("blanks displayName in the character-data export when names are hidden", () => {
    const member = partyMember({ displayName: "Lewd", networkUserId: "abc", networkUserName: "LewdTy" });

    exportCharacterDataToClipboard(member, true);
    expect(JSON.parse(lastWrite()).displayName).toBe("Lewd");

    exportCharacterDataToClipboard(member, false);
    const exported = JSON.parse(lastWrite());
    expect(exported.displayName).toBe("");
    expect(exported.networkUserId).toBeUndefined();
    expect(exported.networkUserName).toBeUndefined();
  });
});
