import { LineChart } from "@mantine/charts";
import { Group, Table, Text } from "@mantine/core";
import { useTranslation } from "react-i18next";

import { ChartTooltip } from "@/components/ChartTooltip";
import { Table as MeterTable } from "@/components/Table";
import {
  SBA_INTERVAL,
  breakAreas,
  buildPlayerLabels,
  confluxBossLines,
  confluxSeamLines,
  linkTimeAreas,
  makeResolvePlayerName,
  sbaLockdownAreas,
} from "@/components/chartCommon";
import {
  type ComputedPlayerState,
  type EncounterState,
  type LinkTimeWindow,
  type PlayerData,
  type SBAEvent,
  type SortDirection,
  type SortType,
} from "@/types";
import { millisecondsToElapsedFormat } from "@/utils/format";

const CHART_HEIGHT = 400;

export type SbaTabProps = {
  playerData: PlayerData[];
  players: ComputedPlayerState[];
  // The whole-encounter state, not the enemy-filtered one.
  encounter: EncounterState;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
  showDisplayNames: boolean;
  streamerMode: boolean;
  playerColors: string[];
  sbaChartLen: number;
  sbaChart: Record<number, number[]>;
  sbaEvents: SBAEvent[];
  linkTimeWindows: LinkTimeWindow[];
  confluxAreaClears: number[];
  confluxBossClears: number[];
  sbaWindows: LinkTimeWindow[];
  showSbaChain: boolean;
  showBreak: boolean;
  breakWindows: LinkTimeWindow[];
  showLinkTime: boolean;
};

export const SbaTab = ({
  playerData,
  players,
  encounter,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
  showDisplayNames,
  streamerMode,
  playerColors,
  sbaChartLen,
  sbaChart,
  sbaEvents,
  linkTimeWindows,
  confluxAreaClears,
  confluxBossClears,
  sbaWindows,
  showSbaChain,
  showBreak,
  breakWindows,
  showLinkTime,
}: SbaTabProps) => {
  const { t } = useTranslation();

  const resolvePlayerName = makeResolvePlayerName(players, playerData, showDisplayNames, streamerMode);

  const sbaData = [];
  for (let i = 0; i < sbaChartLen; i++) {
    const sbaDatapoint: {
      timestamp?: string;
    } & { [key: string]: number } = {};

    const timestamp = i * (SBA_INTERVAL * 1000);

    sbaDatapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);

    for (const playerIndex in sbaChart) {
      const playerName = resolvePlayerName(Number(playerIndex));

      const value = sbaChart[playerIndex][i];
      sbaDatapoint[playerName] = value / 10.0;
    }

    sbaData.push(sbaDatapoint);
  }

  const sbaLabels = buildPlayerLabels(players, playerData, showDisplayNames, streamerMode, playerColors).filter(
    (label) => label.partySlotIndex !== -1
  );

  return (
    <Group mt="20" gap="xs">
      <MeterTable
        encounterState={encounter}
        sortType={sortType}
        sortDirection={sortDirection}
        setSortType={setSortType}
        setSortDirection={setSortDirection}
        partyData={playerData}
        metric="sba"
      />
      <Text size="sm">{t("ui.logs.sba-chart")}</Text>
      <LineChart
        h={CHART_HEIGHT}
        data={sbaData}
        dataKey="timestamp"
        withDots={false}
        withLegend
        series={sbaLabels}
        valueFormatter={(value) => {
          return `${value}%`;
        }}
        tooltipProps={{
          content: ({ label, payload }) => <ChartTooltip label={label} payload={payload} />,
        }}
      >
        {showLinkTime ? linkTimeAreas(linkTimeWindows, SBA_INTERVAL * 1000, sbaData.length) : null}
        {confluxSeamLines(confluxAreaClears, SBA_INTERVAL * 1000, sbaData.length)}
        {showBreak ? breakAreas(breakWindows, SBA_INTERVAL * 1000, sbaData.length) : null}
        {showSbaChain ? sbaLockdownAreas(sbaWindows, SBA_INTERVAL * 1000, sbaData.length) : null}
        {confluxBossLines(confluxBossClears, SBA_INTERVAL * 1000, sbaData.length)}
      </LineChart>
      <Table striped layout="fixed">
        <Table.Tbody>
          {sbaEvents.map((payload, index) => {
            const [timestamp, event] = payload;
            const eventType = Object.keys(event)[0];

            // @ts-expect-error: eventType is dynamic here.
            const playerName = resolvePlayerName(event[eventType].actor_index);

            return (
              <Table.Tr key={index}>
                <Table.Td>
                  <Text size="xs">{millisecondsToElapsedFormat(timestamp)}</Text>
                </Table.Td>
                <Table.Td>
                  <Text size="xs">
                    {playerName} - {t(`ui.sba.${eventType}`)}
                  </Text>
                </Table.Td>
              </Table.Tr>
            );
          })}
        </Table.Tbody>
      </Table>
    </Group>
  );
};
