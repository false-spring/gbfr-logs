import { LineChart } from "@mantine/charts";
import { Group, Select, Table, Text } from "@mantine/core";
import { Fragment, useState } from "react";
import { useTranslation } from "react-i18next";
import { ReferenceDot } from "recharts";

import { ChartTooltip } from "@/components/ChartTooltip";
import { SBA_INTERVAL, buildPlayerLabels, makeResolvePlayerName } from "@/components/chartCommon";
import { HEAL_CATEGORIES, type ComputedPlayerState, type HealBreakdown, type PlayerData } from "@/types";
import { humanizeNumbers, millisecondsToElapsedFormat } from "@/utils/format";

const CHART_HEIGHT = 400;

type MiscMetric = "heal-provided" | "heal-received" | "damage-taken";
const METRICS: MiscMetric[] = ["heal-provided", "heal-received", "damage-taken"];

export type MiscTabProps = {
  playerData: PlayerData[];
  players: ComputedPlayerState[];
  showDisplayNames: boolean;
  streamerMode: boolean;
  playerColors: string[];
  miscChartLen: number;
  healProvidedChart: Record<number, number[]>;
  healReceivedChart: Record<number, number[]>;
  damageTakenChart: Record<number, number[]>;
  // playerId -> death offsets (ms from encounter start), for the skulls.
  deaths: Record<number, number[]>;
};

export const MiscTab = ({
  playerData,
  players,
  showDisplayNames,
  streamerMode,
  playerColors,
  miscChartLen,
  healProvidedChart,
  healReceivedChart,
  damageTakenChart,
  deaths,
}: MiscTabProps) => {
  const { t } = useTranslation();
  const [metric, setMetric] = useState<MiscMetric>("heal-provided");
  const [openRows, setOpenRows] = useState<Set<number>>(new Set());
  const toggleRow = (index: number) =>
    setOpenRows((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });

  const resolvePlayerName = makeResolvePlayerName(players, playerData, showDisplayNames, streamerMode);

  const chart =
    metric === "heal-provided" ? healProvidedChart : metric === "heal-received" ? healReceivedChart : damageTakenChart;

  const intervalMs = SBA_INTERVAL * 1000;

  const data: ({ timestamp: string } & { [key: string]: number | string })[] = [];
  for (let i = 0; i < miscChartLen; i++) {
    const point: { timestamp: string } & { [key: string]: number | string } = {
      timestamp: millisecondsToElapsedFormat(i * intervalMs),
    };
    for (const playerId in chart) {
      point[resolvePlayerName(Number(playerId))] = chart[playerId][i] ?? 0;
    }
    data.push(point);
  }

  const labels = buildPlayerLabels(players, playerData, showDisplayNames, streamerMode, playerColors).filter(
    (label) => label.partySlotIndex !== -1
  );

  // Deaths are floored onto buckets, not rounded: that matches how the backend buckets.
  const lastBucket = Math.max(miscChartLen - 1, 0);
  const skulls = Object.entries(deaths).flatMap(([playerId, offsets]) => {
    const series = chart[Number(playerId)];
    if (!series) return [];
    const name = resolvePlayerName(Number(playerId));
    const color = labels.find((l) => l.name === name)?.color ?? "#e03131";
    return offsets.map((ms, i) => {
      const bucket = Math.min(Math.floor(ms / intervalMs), lastBucket);
      return (
        <ReferenceDot
          key={`death-${playerId}-${i}`}
          x={millisecondsToElapsedFormat(bucket * intervalMs)}
          y={series[bucket] ?? 0}
          yAxisId="left"
          ifOverflow="extendDomain"
          shape={(props: { cx?: number; cy?: number }) => {
            const { cx, cy } = props;
            if (cx == null || cy == null) return <g />;
            return (
              <g style={{ pointerEvents: "none" }}>
                <circle cx={cx} cy={cy} r={9} fill={color} stroke="rgba(0,0,0,0.55)" strokeWidth={1} />
                <text x={cx} y={cy + 0.5} textAnchor="middle" dominantBaseline="central" fontSize={11}>
                  💀
                </text>
              </g>
            );
          }}
        />
      );
    });
  });

  const totalFor = (player: ComputedPlayerState): number =>
    metric === "heal-provided"
      ? player.healDone
      : metric === "heal-received"
        ? player.healReceived
        : player.totalDamageTaken;

  const breakdownFor = (player: ComputedPlayerState): HealBreakdown | null =>
    metric === "heal-provided"
      ? player.healProvidedByType
      : metric === "heal-received"
        ? player.healReceivedByType
        : null;

  const rows = [...players].sort((a, b) => totalFor(b) - totalFor(a));

  const humanized = (n: number) => {
    const [v, unit] = humanizeNumbers(n);
    return `${v}${unit}`;
  };

  return (
    <Group mt="20" gap="xs">
      <Select
        data={METRICS.map((m) => ({ value: m, label: t(`ui.logs.misc-${m}`) }))}
        value={metric}
        onChange={(value) => value && setMetric(value as MiscMetric)}
        allowDeselect={false}
        maw={260}
      />
      <Table striped layout="fixed">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>{t("ui.meter-columns.name")}</Table.Th>
            <Table.Th style={{ textAlign: "right" }}>{t(`ui.logs.misc-${metric}`)}</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((player) => {
            const breakdown = breakdownFor(player);
            const isOpen = openRows.has(player.index);
            const parts = breakdown ? HEAL_CATEGORIES.filter((c) => breakdown[c.key] > 0) : [];
            const expandable = parts.length > 0;
            return (
              <Fragment key={player.index}>
                <Table.Tr
                  style={{ cursor: expandable ? "pointer" : undefined }}
                  onClick={() => expandable && toggleRow(player.index)}
                >
                  <Table.Td>
                    <Text size="xs">
                      {expandable ? (isOpen ? "▾ " : "▸ ") : ""}
                      {resolvePlayerName(player.index)}
                    </Text>
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right" }}>
                    <Text size="xs">{humanized(totalFor(player))}</Text>
                  </Table.Td>
                </Table.Tr>
                {isOpen &&
                  parts.map((c) => (
                    <Table.Tr key={`${player.index}-${c.key}`}>
                      <Table.Td style={{ paddingLeft: 24 }}>
                        <Text size="xs" c="dimmed">
                          {t(`ui.logs.${c.label}`)}
                        </Text>
                      </Table.Td>
                      <Table.Td style={{ textAlign: "right" }}>
                        <Text size="xs" c="dimmed">
                          {humanized(breakdown![c.key])}
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  ))}
              </Fragment>
            );
          })}
        </Table.Tbody>
      </Table>
      <Text size="sm">{t(`ui.logs.misc-${metric}`)}</Text>
      <LineChart
        h={CHART_HEIGHT}
        data={data}
        dataKey="timestamp"
        withDots={false}
        withLegend
        series={labels}
        valueFormatter={(value) => {
          const [v, unit] = humanizeNumbers(value);
          return `${v}${unit}`;
        }}
        tooltipProps={{
          content: ({ label, payload }) => <ChartTooltip label={label} payload={payload} />,
        }}
      >
        {skulls}
      </LineChart>
    </Group>
  );
};
