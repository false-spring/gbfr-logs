import {
  ComputedSbaSourceState,
  ComputedSkillGroup,
  ComputedSkillState,
  MeterColumns,
  SkillMeterColumns,
} from "@/types";
import { humanizeNumbers } from "@/utils/format";
import { ColumnValue } from "./usePlayerRow";

export const METER_DAMAGE_COLUMNS: MeterColumns[] = [
  MeterColumns.TotalDamage,
  MeterColumns.DPS,
  MeterColumns.TotalStunValue,
  MeterColumns.StunPerSecond,
  MeterColumns.DamagePercentage,
];

export const METER_STUN_COLUMNS: MeterColumns[] = [
  MeterColumns.TotalStunValue,
  MeterColumns.StunPerHit,
  MeterColumns.StunPerSecond,
  MeterColumns.StunPercentage,
];

export const DAMAGE_SKILL_COLUMNS: SkillMeterColumns[] = [
  SkillMeterColumns.Hits,
  SkillMeterColumns.TotalDamage,
  SkillMeterColumns.MinDamage,
  SkillMeterColumns.MaxDamage,
  SkillMeterColumns.AverageDamage,
  SkillMeterColumns.DamagePercentage,
];

export const STUN_SKILL_COLUMNS: SkillMeterColumns[] = [
  SkillMeterColumns.TotalStunValue,
  SkillMeterColumns.MinStunValue,
  SkillMeterColumns.MaxStunValue,
  SkillMeterColumns.StunPerHit,
  SkillMeterColumns.StunPerSecond,
  SkillMeterColumns.StunPercentage,
];

export const METER_SBA_COLUMNS: MeterColumns[] = [
  MeterColumns.TotalSbaAdded,
  MeterColumns.SBA,
  MeterColumns.SbaPercentage,
];

// A tick is one gauge delta from the game's update call, not a hit, so there
// is no per-hit min/max/average here.
export const SBA_SKILL_COLUMNS: SkillMeterColumns[] = [
  SkillMeterColumns.SbaTicks,
  SkillMeterColumns.SbaTotalAdded,
  SkillMeterColumns.SbaPerTick,
  SkillMeterColumns.SbaPercentage,
];

export const SKILL_COLUMN_LABEL: Record<SkillMeterColumns, string> = {
  [SkillMeterColumns.Hits]: "Hits",
  [SkillMeterColumns.TotalDamage]: "Total",
  [SkillMeterColumns.MinDamage]: "Min",
  [SkillMeterColumns.MaxDamage]: "Max",
  [SkillMeterColumns.AverageDamage]: "Avg",
  [SkillMeterColumns.DamagePercentage]: "%",
  [SkillMeterColumns.TotalStunValue]: "Stun",
  [SkillMeterColumns.MinStunValue]: "Min",
  [SkillMeterColumns.MaxStunValue]: "Max",
  [SkillMeterColumns.StunPerHit]: "Per Hit",
  [SkillMeterColumns.StunPerSecond]: "SPS",
  [SkillMeterColumns.StunPercentage]: "%",
  [SkillMeterColumns.SbaTicks]: "Ticks",
  [SkillMeterColumns.SbaTotalAdded]: "Gen",
  [SkillMeterColumns.SbaPerTick]: "Per Tick",
  [SkillMeterColumns.SbaPercentage]: "%",
};

// Gauge units: 1000 is a full bar, so /10 renders a percent.
export const getSbaColumnValue = (column: SkillMeterColumns, source: ComputedSbaSourceState): ColumnValue => {
  switch (column) {
    case SkillMeterColumns.SbaTicks:
      return { value: source.ticks };
    case SkillMeterColumns.SbaTotalAdded:
      return { value: (source.totalSbaAdded / 10).toFixed(1), unit: "%" };
    case SkillMeterColumns.SbaPerTick: {
      const perTick = source.ticks === 0 ? 0 : source.totalSbaAdded / source.ticks;
      return { value: (perTick / 10).toFixed(2), unit: "%" };
    }
    case SkillMeterColumns.SbaPercentage:
      return { value: source.percentage.toFixed(0), unit: "%" };
    default:
      return { value: "" };
  }
};

const humanized = (n: number): ColumnValue => {
  const [value, unit] = humanizeNumbers(n);
  return { value, unit };
};

// `stunPerSecondRatio` is the parent player's stunPerSecond / totalStunValue.
export const getSkillColumnValue = (
  column: SkillMeterColumns,
  skill: ComputedSkillState | ComputedSkillGroup,
  showFullValues: boolean,
  stunPerSecondRatio: number
): ColumnValue => {
  switch (column) {
    case SkillMeterColumns.Hits:
      return { value: skill.hits };
    case SkillMeterColumns.TotalDamage:
      return showFullValues ? { value: skill.totalDamage.toLocaleString() } : humanized(skill.totalDamage);
    case SkillMeterColumns.MinDamage:
      if (skill.minDamage == null) return { value: "" };
      return showFullValues ? { value: skill.minDamage.toLocaleString() } : humanized(skill.minDamage);
    case SkillMeterColumns.MaxDamage:
      if (skill.maxDamage == null) return { value: "" };
      return showFullValues ? { value: skill.maxDamage.toLocaleString() } : humanized(skill.maxDamage);
    case SkillMeterColumns.AverageDamage: {
      const average = skill.hits === 0 ? 0 : skill.totalDamage / skill.hits;
      return showFullValues ? { value: average.toLocaleString() } : humanized(average);
    }
    case SkillMeterColumns.DamagePercentage:
      return { value: skill.percentage.toFixed(0), unit: "%" };
    case SkillMeterColumns.TotalStunValue:
      return showFullValues ? { value: skill.totalStunValue.toLocaleString() } : humanized(skill.totalStunValue);
    case SkillMeterColumns.MinStunValue:
      if (skill.minStunValue == null) return { value: "" };
      return showFullValues ? { value: skill.minStunValue.toLocaleString() } : humanized(skill.minStunValue);
    case SkillMeterColumns.MaxStunValue:
      if (skill.maxStunValue == null) return { value: "" };
      return showFullValues ? { value: skill.maxStunValue.toLocaleString() } : humanized(skill.maxStunValue);
    case SkillMeterColumns.StunPerHit: {
      const perHit = skill.stunHits === 0 ? 0 : skill.totalStunValue / skill.stunHits;
      return showFullValues ? { value: perHit.toLocaleString() } : humanized(perHit);
    }
    case SkillMeterColumns.StunPerSecond: {
      const sps = skill.totalStunValue * stunPerSecondRatio;
      return { value: (sps || 0).toLocaleString() };
    }
    case SkillMeterColumns.StunPercentage:
      return { value: skill.percentage.toFixed(0), unit: "%" };
    default:
      return { value: "" };
  }
};
