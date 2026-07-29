import { describe, expect, it } from "vitest";
import { inferWrightstoneFamily, isFlatSummonBonus, overMasteryDisplayValue, summonEquipBonusValue } from "./equipment";

describe("equipment", () => {
  it("overMasteryDisplayValue", () => {
    expect(overMasteryDisplayValue(0x45c65767, 4.0)).toBe(4);
    expect(overMasteryDisplayValue(0x68b39018, 20.0)).toBe(20);
    expect(overMasteryDisplayValue(0x43b7581d, 20.0)).toBe(20);
    expect(overMasteryDisplayValue(0x6cb38ef3, 1.2000000476837158)).toBe(12);
  });

  it("summonEquipBonusValue", () => {
    const chainBurstLadder = [20, 25, 30, 35, 40, 45, 50, 60, 80, 100];
    expect(summonEquipBonusValue(chainBurstLadder, 1, 5)).toBe(45);
    expect(summonEquipBonusValue([0.1, 0.2, 0.3], 10, 2)).toBe(3);
    expect(summonEquipBonusValue(chainBurstLadder, 1, 10)).toBeNull();
    expect(summonEquipBonusValue(undefined, 1, 0)).toBeNull();
  });

  it("isFlatSummonBonus", () => {
    expect(isFlatSummonBonus(0xf0f77bc1)).toBe(true); // Stun Power Up
    expect(isFlatSummonBonus(0xa8900c80)).toBe(true); // Attack Power Up
    expect(isFlatSummonBonus(0x54b09a37)).toBe(false); // Chain Burst Damage Up
  });

  it("inferWrightstoneFamily", () => {
    expect(
      inferWrightstoneFamily([
        { id: 0xf372f096, level: 20 },
        { id: 0xdc584f60, level: 15 },
        { id: 0x95f3fa86, level: 10 },
      ])
    ).toBe("Fortification Wrightstone");
    expect(
      inferWrightstoneFamily([
        { id: 0x6b694d6d, level: 20 },
        { id: 0x95f3fa86, level: 15 },
        { id: 0x24883af3, level: 10 },
      ])
    ).toBe("Sequestration Wrightstone");
    expect(inferWrightstoneFamily([{ id: 0xceb700ee, level: 20 }])).toBe("Dread Wrightstone");
    expect(inferWrightstoneFamily([{ id: 0x8d78a19b, level: 20 }])).toBe("Vitality Wrightstone");
    expect(inferWrightstoneFamily([{ id: 0x95f3fa86, level: 10 }])).toBeNull();
    expect(inferWrightstoneFamily([{ id: 0x887ae0b0, level: 0 }])).toBeNull();
    expect(inferWrightstoneFamily([])).toBeNull();
  });
});
