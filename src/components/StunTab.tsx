import { LineChart } from "@mantine/charts";
import { Box, Stack, Text } from "@mantine/core";
import { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ChartTooltip } from "@/components/ChartTooltip";
import { Table as MeterTable } from "@/components/Table";
import {
  DPS_INTERVAL,
  breakAreas,
  buildPlayerLabels,
  confluxBossLines,
  confluxSeamLines,
  makeResolvePlayerName,
  sbaLockdownAreas,
} from "@/components/chartCommon";
import {
  type ComputedPlayerState,
  type EncounterState,
  type LinkTimeWindow,
  type PlayerData,
  type SortDirection,
  type SortType,
} from "@/types";
import { humanizeNumbers, millisecondsToElapsedFormat } from "@/utils/format";

export type StunTabProps = {
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
  confluxAreaClears: number[];
  confluxBossClears: number[];
  sbaWindows: LinkTimeWindow[];
  showSbaChain: boolean;
  showBreak: boolean;
  breakWindows: LinkTimeWindow[];
  enemyStun: Record<number, number[]>;
  enemyStunResets: number[];
  enemySelect: ReactNode;
};

export const StunTab = ({
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
  confluxAreaClears,
  confluxBossClears,
  sbaWindows,
  showSbaChain,
  showBreak,
  breakWindows,
  enemyStun,
  enemyStunResets,
  enemySelect,
}: StunTabProps) => {
  const { t } = useTranslation();

  const resolvePlayerName = makeResolvePlayerName(players, playerData, showDisplayNames, streamerMode);

  // Cumulative stun, reset to 0 after each break of the selected enemy.
  const stunResetSet = new Set(enemyStunResets);
  const stunTotals: { [name: string]: number } = {};
  const stunData = [];
  for (let i = 0; i < chartLen + 1; i++) {
    const stunDatapoint: {
      timestamp?: string;
      party?: number;
    } & { [key: string]: number } = {};

    const timestamp = i * (DPS_INTERVAL * 1000);

    stunDatapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);
    let partyTotal = 0;

    for (const playerIndex in enemyStun) {
      const playerName = resolvePlayerName(Number(playerIndex));

      stunTotals[playerName] = (stunTotals[playerName] ?? 0) + (enemyStun[playerIndex][i] || 0);
      const rounded = Math.round(stunTotals[playerName]);
      stunDatapoint[playerName] = rounded;
      partyTotal += rounded;
    }

    stunDatapoint["party"] = partyTotal;
    stunData.push(stunDatapoint);

    if (stunResetSet.has(i)) {
      for (const name of Object.keys(stunTotals)) {
        stunTotals[name] = 0;
      }
    }
  }

  const stunLabels = buildPlayerLabels(players, playerData, showDisplayNames, streamerMode, playerColors);

  stunLabels.push({
    name: "party",
    partySlotIndex: -1,
    label: t("ui.logs.cumulative-stun", "Cumulative Stun"),
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
          metric="stun"
        />
        <Text size="sm">{t("ui.logs.cumulative-stun", "Cumulative Stun")}</Text>
        <LineChart
          h={400}
          data={stunData}
          dataKey="timestamp"
          withDots={false}
          withLegend
          series={stunLabels}
          valueFormatter={(value) => {
            const [num, suffix] = humanizeNumbers(value);
            return `${num}${suffix}`;
          }}
          tooltipProps={{
            content: ({ label, payload }) => (
              <ChartTooltip
                label={label}
                payload={payload}
                partyLabel={t("ui.logs.cumulative-stun", "Cumulative Stun")}
              />
            ),
          }}
        >
          {confluxSeamLines(confluxAreaClears, DPS_INTERVAL * 1000, stunData.length)}
          {showBreak ? breakAreas(breakWindows, DPS_INTERVAL * 1000, stunData.length) : null}
          {showSbaChain ? sbaLockdownAreas(sbaWindows, DPS_INTERVAL * 1000, stunData.length) : null}
          {confluxBossLines(confluxBossClears, DPS_INTERVAL * 1000, stunData.length)}
        </LineChart>
      </Stack>
    </Box>
  );
};
