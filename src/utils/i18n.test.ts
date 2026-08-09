import { describe, expect, it, vi } from "vitest";

vi.mock("i18next", () => ({
  t: (keys: string | string[]) => (Array.isArray(keys) ? keys[0] : keys),
}));

import { SkillState } from "@/types";

import { getSkillName } from "./i18n";

const skill = (over: Partial<SkillState>): SkillState => ({
  actionType: { Normal: 1 },
  childCharacterType: "Pl1000",
  hits: 1,
  minDamage: 1,
  maxDamage: 1,
  totalDamage: 1,
  totalStunValue: 0,
  minStunValue: 0,
  maxStunValue: 0,
  stunHits: 0,
  ...over,
});

describe("getSkillName, Conflux auras", () => {
  it("names each aura that shares the 99999 sentinel", () => {
    const cases = [
      ["ToxicBlast", "skills.default.conflux-toxic-blast"],
      ["LusterOfDarkness", "skills.default.conflux-luster-of-darkness"],
      ["ReflectionOfAFallenFort", "skills.default.conflux-reflection-of-a-fallen-fort"],
      ["MysteryBox", "skills.default.conflux-mystery-box"],
      ["IceAndFireFollowUp", "skills.default.conflux-ice-and-fire-follow-up"],
    ] as const;

    for (const [auraSource, expected] of cases) {
      expect(getSkillName("Pl1000", skill({ actionType: { Normal: 99999 }, auraSource }))).toBe(expected);
    }
  });

  it("falls through to the generic label for an unclassified sentinel hit", () => {
    expect(getSkillName("Pl1000", skill({ actionType: { Normal: 99999 }, auraSource: "None" }))).toBe(
      "skills.Pl1000.99999"
    );
  });

  it("falls through when the field is absent entirely", () => {
    expect(getSkillName("Pl1000", skill({ actionType: { Normal: 99999 } }))).toBe("skills.Pl1000.99999");
  });

  it("ignores auraSource on any other action id", () => {
    expect(getSkillName("Pl1000", skill({ actionType: { Normal: 100009 }, auraSource: "ToxicBlast" }))).toBe(
      "skills.Pl1000.100009"
    );
  });
});

describe("getSkillName, the ether gun", () => {
  it("names both of the gun's shots from the shared strings", () => {
    expect(getSkillName("Pl2600", skill({ actionType: { Normal: 5000 }, etherGun: true }))).toBe("skills.default.5000");
    expect(getSkillName("Pl2800", skill({ actionType: { Normal: 5010 }, etherGun: true }))).toBe("skills.default.5010");
  });

  it("leaves the skill sharing the id alone", () => {
    expect(
      getSkillName("Pl2600", skill({ actionType: { Normal: 5000 }, childCharacterType: "Pl2600", etherGun: false }))
    ).toBe("skills.Pl2600.5000");
  });

  it("falls through when the field is absent entirely", () => {
    expect(getSkillName("Pl2600", skill({ actionType: { Normal: 5000 }, childCharacterType: "Pl2600" }))).toBe(
      "skills.Pl2600.5000"
    );
  });

  it("is a no-op for a character with no skill at the id", () => {
    expect(getSkillName("Pl1800", skill({ actionType: { Normal: 5010 }, etherGun: true }))).toBe("skills.default.5010");
  });
});
