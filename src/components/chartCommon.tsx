import { type ComputedPlayerState, type LinkTimeWindow, type PlayerData } from "@/types";
import { resolvePartySlotIndex } from "@/utils/derive";
import { millisecondsToElapsedFormat } from "@/utils/format";
import { translatedPlayerName } from "@/utils/i18n";
import { ReferenceArea, ReferenceLine } from "recharts";

export const DPS_INTERVAL = 3;
export const SBA_INTERVAL = 1;

export const CHART_Y_AXIS_WIDTH = 60;

export type Label = { name: string; partySlotIndex: number; label?: string; color: string; strokeDasharray?: string }[];

export const makeResolvePlayerName =
  (players: ComputedPlayerState[], playerData: PlayerData[], showDisplayNames: boolean, streamerMode: boolean) =>
  (actorIndex: number): string => {
    const player = players.find((p) => p.index === actorIndex);
    const partySlotIndex = resolvePartySlotIndex(playerData, actorIndex);

    return translatedPlayerName(
      partySlotIndex,
      playerData[partySlotIndex],
      player as ComputedPlayerState,
      showDisplayNames && !streamerMode
    );
  };

export const buildPlayerLabels = (
  players: ComputedPlayerState[],
  playerData: PlayerData[],
  showDisplayNames: boolean,
  streamerMode: boolean,
  playerColors: string[]
): Label =>
  players.map((player) => {
    const partySlotIndex = resolvePartySlotIndex(playerData, player.index);
    const color = partySlotIndex !== -1 ? playerColors[partySlotIndex] : playerColors[player.partyIndex];

    return {
      name: translatedPlayerName(partySlotIndex, playerData[partySlotIndex], player, showDisplayNames && !streamerMode),
      damage: player.totalDamage,
      partySlotIndex,
      color,
    };
  });

export type OverlayBand = { startIndex: number; endIndex: number };

// Literal hex: html2canvas cannot resolve CSS variables in SVG attributes.
const LINK_TIME_FILL = "#4dabf7";
const SBA_LOCKDOWN_FILL = "#ffe066";
const BREAK_FILL = "#ced4da";
const STATUS_FILL = "#cc5de8";

export const overlayBands = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number): OverlayBand[] => {
  const lastIndex = bucketCount - 1;
  if (lastIndex < 1 || intervalMs <= 0) return [];

  const bands: OverlayBand[] = [];
  for (const [startMs, endMs] of windows) {
    if (endMs <= startMs) continue;

    const startIndex = Math.min(Math.max(Math.floor(startMs / intervalMs), 0), lastIndex);
    const endIndex = Math.min(Math.ceil(endMs / intervalMs), lastIndex);
    if (endIndex <= startIndex) continue;

    bands.push({ startIndex, endIndex });
  }

  bands.sort((a, b) => a.startIndex - b.startIndex);

  return bands.reduce<OverlayBand[]>((merged, band) => {
    const previous = merged[merged.length - 1];
    if (previous && band.startIndex <= previous.endIndex) {
      previous.endIndex = Math.max(previous.endIndex, band.endIndex);
      return merged;
    }
    return [...merged, { ...band }];
  }, []);
};

export type OverlaySpan = { left: number; width: number };

// Spans narrower than this fraction of the plot paint nothing; shorter windows get widened to it.
export const MIN_OVERLAY_SPAN = 0.0025;

export const overlaySpans = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number): OverlaySpan[] => {
  const spanMs = (bucketCount - 1) * intervalMs;
  if (spanMs <= 0) return [];

  const minMs = spanMs * MIN_OVERLAY_SPAN;

  const clamped: LinkTimeWindow[] = [];
  for (const [startMs, endMs] of windows) {
    if (endMs <= startMs) continue;

    const start = Math.min(Math.max(startMs, 0), spanMs);
    const end = Math.min(Math.max(endMs, 0), spanMs);
    if (end <= start) continue;

    if (end - start >= minMs) {
      clamped.push([start, end]);
      continue;
    }

    const flooredEnd = Math.min(start + minMs, spanMs);
    clamped.push([Math.max(flooredEnd - minMs, 0), flooredEnd]);
  }

  clamped.sort((a, b) => a[0] - b[0]);

  const merged = clamped.reduce<LinkTimeWindow[]>((spans, [start, end]) => {
    const previous = spans[spans.length - 1];
    if (previous && start <= previous[1]) {
      previous[1] = Math.max(previous[1], end);
      return spans;
    }
    return [...spans, [start, end]];
  }, []);

  return merged.map(([start, end]) => ({ left: start / spanMs, width: (end - start) / spanMs }));
};

// yAxisId="left" is load-bearing: ReferenceArea/ReferenceLine default to axis
// id 0, which the Mantine charts do not have, and silently draw nothing on a miss.
const overlayAreas = (
  windows: LinkTimeWindow[],
  intervalMs: number,
  bucketCount: number,
  { fill, fillOpacity, keyPrefix }: { fill: string; fillOpacity: number; keyPrefix: string }
) =>
  overlayBands(windows, intervalMs, bucketCount).map(({ startIndex, endIndex }) => (
    <ReferenceArea
      key={`${keyPrefix}-${startIndex}`}
      x1={millisecondsToElapsedFormat(startIndex * intervalMs)}
      x2={millisecondsToElapsedFormat(endIndex * intervalMs)}
      yAxisId="left"
      fill={fill}
      fillOpacity={fillOpacity}
      ifOverflow="hidden"
    />
  ));

export const linkTimeAreas = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number) =>
  overlayAreas(windows, intervalMs, bucketCount, { fill: LINK_TIME_FILL, fillOpacity: 0.15, keyPrefix: "link" });

const CONFLUX_SEAM_STROKE = "#868e96";
const CONFLUX_BOSS_STROKE = "#adb5bd";

export const confluxSeamBuckets = (clears: number[], intervalMs: number, bucketCount: number): number[] => {
  const lastIndex = bucketCount - 1;
  if (lastIndex < 1 || intervalMs <= 0) return [];

  const buckets = clears.map((ms) => Math.ceil(ms / intervalMs)).filter((index) => index > 0 && index < lastIndex);

  return [...new Set(buckets)].sort((a, b) => a - b);
};

export const confluxSeamLines = (clears: number[], intervalMs: number, bucketCount: number) =>
  confluxSeamBuckets(clears, intervalMs, bucketCount).map((index) => (
    <ReferenceLine
      key={index}
      x={millisecondsToElapsedFormat(index * intervalMs)}
      yAxisId="left"
      stroke={CONFLUX_SEAM_STROKE}
      strokeDasharray="4 4"
      strokeOpacity={0.5}
      ifOverflow="hidden"
    />
  ));

export const confluxBossLines = (clears: number[], intervalMs: number, bucketCount: number) =>
  confluxSeamBuckets(clears, intervalMs, bucketCount).map((index) => (
    <ReferenceLine
      key={`boss-${index}`}
      x={millisecondsToElapsedFormat(index * intervalMs)}
      yAxisId="left"
      stroke={CONFLUX_BOSS_STROKE}
      strokeOpacity={0.75}
      ifOverflow="hidden"
    />
  ));

// Clamps the DPS smoothing window to the last seam. `seams` must be sorted ascending.
export const rollingWindowStart = (bucket: number, seams: number[]): number => {
  let start = 0;
  for (const seam of seams) {
    if (seam > bucket) break;
    start = seam;
  }
  return start;
};

export const sbaLockdownAreas = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number) =>
  overlayAreas(windows, intervalMs, bucketCount, { fill: SBA_LOCKDOWN_FILL, fillOpacity: 0.1, keyPrefix: "sba" });

export const breakAreas = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number) =>
  overlayAreas(windows, intervalMs, bucketCount, { fill: BREAK_FILL, fillOpacity: 0.08, keyPrefix: "break" });

// The optional props arrive from ReferenceArea cloning its measured plot rect into `shape`.
type OverlaySpanRectsProps = {
  spans: OverlaySpan[];
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  fill?: string;
  fillOpacity?: number;
  clipPath?: string;
};

const OverlaySpanRects = ({
  spans,
  x = 0,
  y = 0,
  width = 0,
  height = 0,
  fill,
  fillOpacity,
  clipPath,
}: OverlaySpanRectsProps) => (
  <g clipPath={clipPath}>
    {spans.map((span) => (
      <rect
        key={span.left}
        x={x + span.left * width}
        y={y}
        width={span.width * width}
        height={height}
        fill={fill}
        fillOpacity={fillOpacity}
      />
    ))}
  </g>
);

// Status windows are routinely shorter than a bucket, which a category-bounded
// ReferenceArea cannot span, so a custom shape draws them from raw milliseconds.
export const statusAreas = (windows: LinkTimeWindow[], intervalMs: number, bucketCount: number) => {
  const spans = overlaySpans(windows, intervalMs, bucketCount);
  if (spans.length === 0) return [];

  return [
    <ReferenceArea
      key="status"
      yAxisId="left"
      fill={STATUS_FILL}
      fillOpacity={0.22}
      ifOverflow="hidden"
      shape={<OverlaySpanRects spans={spans} />}
    />,
  ];
};

export const OVERLAY_TRACK_COLORS = {
  linkTime: LINK_TIME_FILL,
  sbaChain: SBA_LOCKDOWN_FILL,
  break: BREAK_FILL,
} as const;

export type OverlayTrackSegment = { left: number; width: number };

export const overlayTrackSegments = (
  windows: LinkTimeWindow[],
  intervalMs: number,
  bucketCount: number
): OverlayTrackSegment[] =>
  overlaySpans(windows, intervalMs, bucketCount).map(({ left, width }) => ({
    left: left * 100,
    width: width * 100,
  }));
