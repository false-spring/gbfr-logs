import { LineChart } from "@mantine/charts";
import { Box, Chip, Group, Stack, Text } from "@mantine/core";
import { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ChartTooltip } from "@/components/ChartTooltip";
import { OverlayTracks, type OverlayTrack } from "@/components/OverlayTracks";
import { StackDepthChart } from "@/components/StackDepthChart";
import { StatusMagnitudeChart } from "@/components/StatusMagnitudeChart";
import { Table as MeterTable } from "@/components/Table";
import {
  CHART_Y_AXIS_WIDTH,
  DPS_INTERVAL,
  OVERLAY_TRACK_COLORS,
  breakAreas,
  buildPlayerLabels,
  confluxBossLines,
  confluxSeamBuckets,
  confluxSeamLines,
  linkTimeAreas,
  makeResolvePlayerName,
  rollingWindowStart,
  sbaLockdownAreas,
  statusAreas,
} from "@/components/chartCommon";
import {
  type ComputedPlayerState,
  type EncounterState,
  type LinkTimeWindow,
  type PlayerData,
  type SortDirection,
  type SortType,
  type StatusStackSample,
  type StatusValueSample,
} from "@/types";
import { humanizeNumbers, millisecondsToElapsedFormat } from "@/utils/format";

const CHART_HEIGHT = 400;

export type OverviewTabProps = {
  encounter: EncounterState;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
  playerData: PlayerData[];
  players: ComputedPlayerState[];
  showDisplayNames: boolean;
  streamerMode: boolean;
  playerColors: string[];
  chartLen: number;
  enemyDps: Record<number, number[]>;
  linkTimeWindows: LinkTimeWindow[];
  confluxAreaClears: number[];
  confluxBossClears: number[];
  sbaWindows: LinkTimeWindow[];
  breakWindows: LinkTimeWindow[];
  showLinkTime: boolean;
  showSbaChain: boolean;
  showBreak: boolean;
  setOverlayVisible: (overlay: "link-time" | "sba-chain" | "break", visible: boolean) => void;
  enemySelect: ReactNode;
  statusSelect: ReactNode;
  statusWindows: LinkTimeWindow[];
  statusStackSamples: StatusStackSample[];
  statusValueSamples: StatusValueSample[];
  statusValueIsFraction: boolean | undefined;
};

export const OverviewTab = ({
  encounter,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
  playerData,
  players,
  showDisplayNames,
  streamerMode,
  playerColors,
  chartLen,
  enemyDps,
  linkTimeWindows,
  confluxAreaClears,
  confluxBossClears,
  sbaWindows,
  breakWindows,
  showLinkTime,
  showSbaChain,
  showBreak,
  setOverlayVisible,
  enemySelect,
  statusSelect,
  statusWindows,
  statusStackSamples,
  statusValueSamples,
  statusValueIsFraction,
}: OverviewTabProps) => {
  const { t } = useTranslation();

  const resolvePlayerName = makeResolvePlayerName(players, playerData, showDisplayNames, streamerMode);

  const hasLinkTime = linkTimeWindows.length > 0;
  const hasSbaChain = sbaWindows.length > 0;
  const hasBreak = breakWindows.length > 0;
  const overlayLabel = (label: string, available: boolean) =>
    available ? label : `${label} (${t("ui.logs.overlay-unavailable", "N/A")})`;

  // A selected status takes the plot area over; the other overlays collapse
  // into labelled tracks above the chart.
  const statusSelected = statusWindows.length > 0;
  const overlayTracks: OverlayTrack[] = [];
  if (hasLinkTime && showLinkTime) {
    const label = t("ui.logs.overlay-link-time", "Link Time");
    overlayTracks.push({ label, color: OVERLAY_TRACK_COLORS.linkTime, windows: linkTimeWindows });
  }
  if (hasSbaChain && showSbaChain) {
    const label = t("ui.logs.overlay-sba-chain", "SBA Chain");
    overlayTracks.push({ label, color: OVERLAY_TRACK_COLORS.sbaChain, windows: sbaWindows });
  }
  if (hasBreak && showBreak) {
    const label = t("ui.logs.overlay-break", "Break");
    overlayTracks.push({ label, color: OVERLAY_TRACK_COLORS.break, windows: breakWindows });
  }

  const seamBuckets = confluxSeamBuckets(confluxAreaClears, DPS_INTERVAL * 1000, chartLen + 1);

  const data = [];
  for (let i = 0; i < chartLen + 1; i++) {
    const datapoint: {
      timestamp?: string;
      party?: number;
    } & { [key: string]: number } = {};

    const timestamp = i * (DPS_INTERVAL * 1000);

    datapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);
    datapoint["party"] = 0;

    for (const playerIndex in enemyDps) {
      const playerName = resolvePlayerName(Number(playerIndex));

      const lastFiveValues = enemyDps[playerIndex].slice(Math.max(i - 5, rollingWindowStart(i, seamBuckets)), i);
      const totalLastFiveValues = lastFiveValues.reduce((a, b) => a + b, 0);
      const currentValue = enemyDps[playerIndex][i] || 0;
      const averageValue = (totalLastFiveValues + currentValue) / (lastFiveValues.length + 1);

      const value = Math.round(averageValue / DPS_INTERVAL);
      datapoint[playerName] = value;
      datapoint["party"] += value;
    }

    data.push(datapoint);
  }

  // The status charts plot against this same span on a numeric axis, which
  // keeps them aligned with the damage chart's category axis.
  const statusExtentMs = Math.max(0, data.length - 1) * DPS_INTERVAL * 1000;

  const labels = buildPlayerLabels(players, playerData, showDisplayNames, streamerMode, playerColors);

  labels.push({
    name: "party",
    partySlotIndex: -1,
    label: t("ui.logs.damage-per-second"),
    color: "grey",
    strokeDasharray: "2 2",
  });

  return (
    <Box mt="md">
      <Stack>
        {enemySelect}
        <MeterTable
          encounterState={encounter}
          sortType={sortType}
          sortDirection={sortDirection}
          setSortType={setSortType}
          setSortDirection={setSortDirection}
          partyData={playerData}
        />
        <Group gap="xs" align="center">
          <Text size="sm">{t("ui.logs.damage-per-second")}</Text>
          <Chip
            size="xs"
            color="blue"
            checked={showLinkTime && hasLinkTime}
            disabled={!hasLinkTime}
            onChange={(checked) => setOverlayVisible("link-time", checked)}
          >
            {overlayLabel(t("ui.logs.overlay-link-time", "Link Time"), hasLinkTime)}
          </Chip>
          <Chip
            size="xs"
            color="yellow"
            checked={showSbaChain && hasSbaChain}
            disabled={!hasSbaChain}
            onChange={(checked) => setOverlayVisible("sba-chain", checked)}
          >
            {overlayLabel(t("ui.logs.overlay-sba-chain", "SBA Chain"), hasSbaChain)}
          </Chip>
          <Chip
            size="xs"
            color="gray"
            checked={showBreak && hasBreak}
            disabled={!hasBreak}
            onChange={(checked) => setOverlayVisible("break", checked)}
          >
            {overlayLabel(t("ui.logs.overlay-break", "Break"), hasBreak)}
          </Chip>
        </Group>
        {statusSelected && overlayTracks.length > 0 ? (
          <OverlayTracks tracks={overlayTracks} intervalMs={DPS_INTERVAL * 1000} bucketCount={data.length} />
        ) : null}
        <LineChart
          h={CHART_HEIGHT}
          data={data}
          dataKey="timestamp"
          withDots={false}
          withLegend
          series={labels}
          yAxisProps={{ width: CHART_Y_AXIS_WIDTH }}
          valueFormatter={(value) => {
            const [num, suffix] = humanizeNumbers(value);
            return `${num}${suffix}`;
          }}
          tooltipProps={{
            content: ({ label, payload }) => <ChartTooltip label={label} payload={payload} />,
          }}
        >
          {statusSelected ? statusAreas(statusWindows, DPS_INTERVAL * 1000, data.length) : null}
          {!statusSelected && showLinkTime ? linkTimeAreas(linkTimeWindows, DPS_INTERVAL * 1000, data.length) : null}
          {confluxSeamLines(confluxAreaClears, DPS_INTERVAL * 1000, data.length)}
          {!statusSelected && showBreak ? breakAreas(breakWindows, DPS_INTERVAL * 1000, data.length) : null}
          {!statusSelected && showSbaChain ? sbaLockdownAreas(sbaWindows, DPS_INTERVAL * 1000, data.length) : null}
          {confluxBossLines(confluxBossClears, DPS_INTERVAL * 1000, data.length)}
        </LineChart>
        <StackDepthChart samples={statusStackSamples} extentMs={statusExtentMs} parentHeight={CHART_HEIGHT} />
        <StatusMagnitudeChart
          samples={statusValueSamples}
          isFraction={statusValueIsFraction}
          extentMs={statusExtentMs}
          parentHeight={CHART_HEIGHT}
        />
        {statusSelect ? (
          <Stack gap={4}>
            <Text size="sm">{t("ui.logs.status-effects", "Status Effects")}</Text>
            {statusSelect}
          </Stack>
        ) : null}
      </Stack>
    </Box>
  );
};
