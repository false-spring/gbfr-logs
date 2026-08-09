import FlatOvermasteryIds from "@/assets/flat-overmastery-ids";
import { toHashString } from "@/utils/format";

// The stun-family params display at 10x their stored per-rank value.
const OVER_MASTERY_DISPLAY_MULTIPLIERS: Record<number, number> = {
  0x59fbb7d8: 10,
  0x6cb38ef3: 10,
  0xa3545ca1: 10,
};

export const overMasteryDisplayValue = (id: number, value: number): number => {
  return Math.round(value * (OVER_MASTERY_DISPLAY_MULTIPLIERS[id] ?? 1));
};

export const isFlatOvermasteryValue = (id: number): boolean => FlatOvermasteryIds.has(id);

const FLAT_SUMMON_BONUS_IDS = [
  0xa3e537b1,
  0xa8900c80, // Attack Power Up
  0xbc4e92cb,
  0xf2256e44, // Health Up
  0xf0f77bc1,
  0xf7b0316f, // Stun Power Up
];

export const isFlatSummonBonus = (id: number): boolean => FLAT_SUMMON_BONUS_IDS.includes(id);

export const summonEquipBonusValue = (
  ladder: number[] | undefined,
  displayMult: number,
  level: number
): number | null => {
  if (!ladder || level < 0 || level >= ladder.length) return null;
  return Math.round(ladder[level] * displayMult);
};

// Each wrightstone family imbues a unique signature primary trait at the highest T.Lvl.
const WRIGHTSTONE_FAMILY_BY_PRIMARY: Record<string, string> = {
  ceb700ee: "Dread Wrightstone", // Stun Power (SKILL_004)
  "8d78a19b": "Vitality Wrightstone", // Critical Hit Rate (SKILL_003)
  f372f096: "Fortification Wrightstone", // HP (SKILL_001)
  "6b694d6d": "Sequestration Wrightstone", // Weak Point DMG (SKILL_014)
};

export const inferWrightstoneFamily = (
  pairs: { id: number | undefined; level: number | undefined }[]
): string | null => {
  let best: { id: number; level: number } | null = null;
  for (const p of pairs) {
    const id = p.id ?? 0;
    const level = p.level ?? 0;
    if (id === 0 || id === 0x887ae0b0) continue; // 0 / empty sentinel
    if (best === null || level > best.level) best = { id, level };
  }
  if (best === null) return null;
  return WRIGHTSTONE_FAMILY_BY_PRIMARY[toHashString(best.id)] ?? null;
};

const CATEGORY_TINT: Record<number, string> = {
  0: "rgba(128, 128, 128, 0.15)",
  1: "rgba(240, 140, 0, 0.16)",
  2: "rgba(0, 190, 210, 0.16)",
  3: "rgba(224, 49, 49, 0.18)",
  4: "rgba(132, 94, 247, 0.22)",
};
export const categoryTint = (category: number): string | undefined => CATEGORY_TINT[category];
