import StatusCatalog from "@/assets/status-catalog";
import StatusClassNames from "@/assets/status-class-names";
import StatusMasterTraitCards from "@/assets/status-master-trait-cards";
import StatusMasterTraitSources from "@/assets/status-master-trait-sources";
import {
  type StatusIntervals,
  type StatusPeakStacks,
  type StatusSourceWindows,
  type StatusSources,
  type StatusStackSample,
  type StatusStackSeries,
  type StatusValueSample,
  type StatusValueSeries,
} from "@/types";
import { PLAYER_ID_BASE } from "@/utils/derive";

export type StatusOption = { value: string; label: string };

export type StatusPolarity = "buff" | "debuff" | "other";

export type StatusOptionGroup = { polarity: StatusPolarity; items: StatusOption[] };

export const statusDisplayName = (statusId: number): string => {
  const catalogued = StatusCatalog[statusId]?.name;
  if (catalogued) return catalogued;

  const fallback = StatusClassNames[statusId];
  if (fallback) return fallback.name ?? fallback.className;

  return `Status 0x${statusId.toString(16)}`;
};

export const statusPolarity = (statusId: number): StatusPolarity => {
  const entry = StatusCatalog[statusId];
  if (entry) return entry.beneficial ? "buff" : "debuff";

  const fallback = StatusClassNames[statusId];
  if (fallback?.beneficial !== undefined) return fallback.beneficial ? "buff" : "debuff";

  return "other";
};

export const buildStatusEntityOptions = (
  intervals: StatusIntervals,
  resolveLabel: (actorId: number) => string
): StatusOption[] => {
  const actors = Object.keys(intervals)
    .map(Number)
    .filter((actorId) => Object.values(intervals[actorId] ?? {}).some((windows) => windows.length > 0));

  return actors
    .map((actorId) => ({ value: String(actorId), label: resolveLabel(actorId) }))
    .sort((a, b) => {
      const aParty = Number(a.value) >= PLAYER_ID_BASE;
      const bParty = Number(b.value) >= PLAYER_ID_BASE;
      if (aParty !== bParty) return aParty ? -1 : 1;
      if (aParty) return Number(a.value) - Number(b.value);
      return a.label.localeCompare(b.label);
    });
};

export const statusLabelWithStacks = (statusId: number, peak: number | undefined): string => {
  const name = statusDisplayName(statusId);
  return peak !== undefined && peak > 1 ? `${name} (×${peak})` : name;
};

export const buildStatusOptionGroups = (
  intervals: StatusIntervals,
  actorId: number | null,
  peakStacks: StatusPeakStacks = {}
): StatusOptionGroup[] => {
  if (actorId === null) return [];

  const byPolarity: Record<StatusPolarity, StatusOption[]> = { buff: [], debuff: [], other: [] };
  for (const [statusId, windows] of Object.entries(intervals[actorId] ?? {})) {
    if (windows.length === 0) continue;
    byPolarity[statusPolarity(Number(statusId))].push({
      value: statusId,
      label: statusLabelWithStacks(Number(statusId), peakStacks[actorId]?.[Number(statusId)]),
    });
  }

  const order: StatusPolarity[] = ["buff", "debuff", "other"];
  return order
    .filter((polarity) => byPolarity[polarity].length > 0)
    .map((polarity) => ({
      polarity,
      items: byPolarity[polarity].sort((a, b) => a.label.localeCompare(b.label)),
    }));
};

export const selectedStatusWindows = (intervals: StatusIntervals, actorId: number | null, statusId: number | null) =>
  actorId === null || statusId === null ? [] : intervals[actorId]?.[statusId] ?? [];

export const selectedStackSamples = (
  series: StatusStackSeries,
  actorId: number | null,
  statusId: number | null
): StatusStackSample[] => (actorId === null || statusId === null ? [] : series[actorId]?.[statusId] ?? []);

export const STATUS_KINDS_WITHOUT_MAGNITUDE = new Set<number>([
  // 102 = Embrasque Unleashed (Beatrix)
  102,
]);

export const selectedValueSamples = (
  series: StatusValueSeries,
  actorId: number | null,
  statusId: number | null
): StatusValueSample[] => (actorId === null || statusId === null ? [] : series[actorId]?.[statusId] ?? []);

export type StatusChartPoint = { t: number; value: number };

export const statusPointsForChart = (samples: StatusValueSample[], extentMs: number): StatusChartPoint[] => {
  if (extentMs <= 0) return [];

  const points: StatusChartPoint[] = [];
  for (const [at, value] of samples) {
    if (at > extentMs) break;
    if (points.length > 0 && points[points.length - 1].value === value) continue;
    // The last of several changes inside one millisecond is what stood.
    if (points.length > 0 && points[points.length - 1].t === at) {
      points[points.length - 1] = { t: at, value };
      continue;
    }
    points.push({ t: at, value });
  }

  if (points.length === 0) return [];

  const last = points[points.length - 1];
  if (last.t < extentMs) points.push({ t: extentMs, value: last.value });

  return points;
};

export const STATUS_SOURCE_ALL = "all";

export const statusSourceKey = (source: StatusSourceWindows): string =>
  `${source.applierIndex ?? "?"}:${source.sourceIds?.join(",") ?? "?"}`;

export const selectedStatusSources = (
  sources: StatusSources,
  actorId: number | null,
  statusId: number | null
): StatusSourceWindows[] => (actorId === null || statusId === null ? [] : sources[actorId]?.[statusId] ?? []);

export const resolveSelectedOption = (options: StatusOption[], selected: string): string =>
  options.some((option) => option.value === selected) ? selected : options[0]?.value ?? STATUS_SOURCE_ALL;

// `sourceIds[0]` identifies the AFFECTED entity, `(type << 16) | BCD4(class)`,
// and enemy-applied statuses repeat the class number in the action slot.
const actionIdIsOnlyAnIdentity = (source: StatusSourceWindows): boolean => {
  const identity = source.sourceIds?.[0];
  const actionId = source.sourceIds?.[1];
  const applier = source.applierIndex;
  if (identity === undefined || actionId === undefined) return false;
  if (applier === null || applier === undefined || applier >= PLAYER_ID_BASE) return false;
  const ENEMY = 2;
  return identity >>> 16 === ENEMY && actionId === (identity & 0xffff);
};

const formatMagnitude = (value: number, valueIsFraction: boolean | undefined): string => {
  if (!valueIsFraction) return Math.round(value).toLocaleString();
  const percent = value * 100;
  return `${Number.isInteger(percent) ? percent : percent.toFixed(1)}%`;
};

// `sourceIds[1]` is a per-character skill id in the damage rows' namespace;
// sigils, traits and passives carry GLOBAL ids, resolved through `status-source-names`.
export const buildStatusSourceOptions = (
  sources: StatusSourceWindows[],
  resolveLabel: (actorId: number) => string,
  allLabel: string,
  unknownLabel: string,
  resolveAction?: (applierIndex: number, actionId: number, magnitude: number | undefined) => string | null,
  valueIsFraction?: boolean
): StatusOption[] => {
  const applierOf = (source: StatusSourceWindows): number | null => source.applierIndex ?? null;
  const magnitudeOf = (source: StatusSourceWindows): number | undefined =>
    source.values.find(([, value]) => value !== 0)?.[1];

  const perApplier = new Map<number | null, number>();
  const magnitudesPerApplier = new Map<number | null, Set<number>>();
  for (const source of sources) {
    const applier = applierOf(source);
    perApplier.set(applier, (perApplier.get(applier) ?? 0) + 1);
    const magnitude = magnitudeOf(source);
    if (magnitude !== undefined) {
      const seen = magnitudesPerApplier.get(applier) ?? new Set<number>();
      seen.add(magnitude);
      magnitudesPerApplier.set(applier, seen);
    }
  }

  const usedSoFar = new Map<number | null, number>();
  const items = sources.map((source) => {
    const applier = applierOf(source);
    const name = applier === null ? unknownLabel : resolveLabel(applier);
    const total = perApplier.get(applier) ?? 1;
    const ordinal = (usedSoFar.get(applier) ?? 0) + 1;
    usedSoFar.set(applier, ordinal);

    const actionId = source.sourceIds?.[1];
    const magnitude = magnitudeOf(source);
    const action =
      applier !== null && actionId !== undefined && resolveAction ? resolveAction(applier, actionId, magnitude) : null;
    if (action) return { value: statusSourceKey(source), label: `${name}: ${action}` };

    // 0 and 0xFFFFFFFF (the engine's unset sentinel) both mean "no action recorded".
    const NO_ACTION = new Set([0, 0xffffffff]);
    if (actionId !== undefined && !NO_ACTION.has(actionId) && !actionIdIsOnlyAnIdentity(source)) {
      return { value: statusSourceKey(source), label: `${name} (#${actionId})` };
    }

    const distinctMagnitudes = magnitudesPerApplier.get(applier)?.size ?? 0;
    if (total > 1 && magnitude !== undefined && distinctMagnitudes === total) {
      return {
        value: statusSourceKey(source),
        label: `${name} (${formatMagnitude(magnitude, valueIsFraction)})`,
      };
    }

    return {
      value: statusSourceKey(source),
      label: total > 1 ? `${name} (${ordinal})` : name,
    };
  });

  return items.length > 1 ? [{ value: STATUS_SOURCE_ALL, label: allLabel }, ...items] : items;
};

// Master traits carry no source id, so the grantor is matched on its declared
// magnitude. The table stores a fraction kind as a PERCENT (10 for 10%), the
// meter as a fraction (0.10); counts are absolute on both sides.
export const masterTraitSourceFor = (
  characterType: string | undefined,
  statusKind: number | null,
  magnitude: number | undefined,
  isFraction: boolean | undefined
): string | null => {
  if (!characterType || statusKind === null) return null;

  const character = characterType.toUpperCase();

  // Card rows declare no value, so they skip the magnitude guard.
  const card = StatusMasterTraitCards[`${character}:${statusKind}`];
  if (card) return card;

  if (magnitude === undefined) return null;

  const candidates = StatusMasterTraitSources[`${character}:${statusKind}`];
  if (!candidates) return null;

  const matches = candidates.filter((candidate) => {
    const declared = isFraction ? candidate.value / 100 : candidate.value;
    const scale = Math.max(Math.abs(declared), Math.abs(magnitude), 1);
    return Math.abs(declared - magnitude) / scale < 1e-3;
  });

  const labels = [...new Set(matches.map((candidate) => candidate.label))];
  return labels.length > 0 ? labels.join(" / ") : null;
};

export const formatStatusValue = (value: number, isFraction: boolean | undefined): string =>
  isFraction ? `${(value * 100).toFixed(1)}%` : value.toLocaleString(undefined, { maximumFractionDigits: 2 });
