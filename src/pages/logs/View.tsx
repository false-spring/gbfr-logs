import {
  Box,
  Button,
  Divider,
  Text,
  Stack,
  NumberFormatter,
  MultiSelect,
  Menu,
  ActionIcon,
  Flex,
  Paper,
  Tabs,
  Group,
  Table,
} from "@mantine/core";
import { LineChart } from "@mantine/charts";
import { ClipboardText } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { t } from "i18next";
import toast from "react-hot-toast";

import { Table as MeterTable } from "../../components/Table";
import {
  EMPTY_ID,
  PLAYER_COLORS,
  epochToLocalTime,
  exportFullEncounterToClipboard,
  exportSimpleEncounterToClipboard,
  formatInPartyOrder,
  humanizeNumbers,
  millisecondsToElapsedFormat,
  toHash,
  translateQuestId,
  translateSigilId,
  translateTraitId,
  translatedPlayerName,
} from "../../utils";
import { ComputedPlayerState, EnemyType, SortDirection, SortType } from "../../types";
import { useEncounterStore, EncounterStateResponse } from "../Logs";

interface ChartTooltipProps {
  label: string;
  payload: Record<string, any>[] | undefined; // eslint-disable-line
}

export const ChartTooltip = ({ label, payload }: ChartTooltipProps) => {
  if (!payload) return null;

  return (
    <Paper px="md" py="sm" withBorder shadow="md" radius="md">
      <Text fw={500} mb={5}>
        {label}
      </Text>
      {payload.map(
        (
          item: any // eslint-disable-line
        ) => (
          <Text key={item.name} fz="sm">
            <Text component="span" c={item.color}>
              {item.name}
            </Text>
            : {new Intl.NumberFormat("en-US").format(item.value)}
          </Text>
        )
      )}
    </Paper>
  );
};

const DPS_INTERVAL = 3;

export const ViewPage = () => {
  const { id } = useParams();
  const {
    encounter,
    dpsChart,
    chartLen,
    targets,
    selectedTargets,
    questId,
    questTimer,
    playerData,
    setSelectedTargets,
    loadFromResponse,
  } = useEncounterStore((state) => ({
    encounter: state.encounterState,
    dpsChart: state.dpsChart,
    chartLen: state.chartLen,
    targets: state.targets,
    selectedTargets: state.selectedTargets,
    playerData: state.players,
    questId: state.questId,
    questTimer: state.questTimer,
    setSelectedTargets: state.setSelectedTargets,
    loadFromResponse: state.loadFromResponse,
  }));
  const [sortType, setSortType] = useState<SortType>("damage");
  const [sortDirection, setSortDirection] = useState<SortDirection>("desc");

  useEffect(() => {
    invoke("fetch_encounter_state", { id: Number(id), options: { targets: selectedTargets } })
      .then((result) => {
        loadFromResponse(result as EncounterStateResponse);
      })
      .catch((e) => {
        toast.error(`Failed to fetch encounter state: ${e}`);
      });
  }, [id, selectedTargets]);

  const handleSimpleEncounterCopy = useCallback(() => {
    if (encounter) exportSimpleEncounterToClipboard(sortType, sortDirection, encounter, playerData);
  }, [sortType, sortDirection, encounter]);

  const handleFullEncounterCopy = useCallback(() => {
    if (encounter) exportFullEncounterToClipboard(sortType, sortDirection, encounter, playerData);
  }, [sortType, sortDirection, encounter]);

  const exportDamageLogToFile = useCallback(() => {
    if (id) invoke("export_damage_log_to_file", { id: Number(id), options: { targets: selectedTargets } });
  }, [id, selectedTargets]);

  if (!encounter) {
    return (
      <Box>
        <Text>
          <Link to="/logs">{t("ui.back-btn")}</Link>
        </Text>
        <Divider my="sm" />
        <Text>Loading...</Text>
      </Box>
    );
  }

  const data = [];

  const players = formatInPartyOrder(encounter.party);

  for (let i = 0; i < chartLen; i++) {
    const datapoint: {
      timestamp?: string;
    } & { [key: string]: number } = {};

    const timestamp = i * (DPS_INTERVAL * 1000);

    datapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);

    for (const playerIndex in dpsChart) {
      const player = players.find((p) => p.index === Number(playerIndex));
      const partySlotIndex = playerData.findIndex((partyMember) => partyMember?.actorIndex === player?.index);
      const playerName = translatedPlayerName(
        partySlotIndex,
        playerData[partySlotIndex],
        player as ComputedPlayerState
      );

      const lastFiveValues = dpsChart[playerIndex].slice(i - 5, i);
      const totalLastFiveValues = lastFiveValues.reduce((a, b) => a + b, 0);
      const currentValue = dpsChart[playerIndex][i] || 0;
      const averageValue = (totalLastFiveValues + currentValue) / (lastFiveValues.length + 1);

      datapoint[playerName] = Math.round(averageValue / DPS_INTERVAL);
    }

    data.push(datapoint);
  }

  const labels = players.map((player) => {
    const partySlotIndex = playerData.findIndex((partyMember) => partyMember?.actorIndex === player.index);

    return {
      name: translatedPlayerName(partySlotIndex, playerData[partySlotIndex], player),
      damage: player.totalDamage,
      color: PLAYER_COLORS[player.partyIndex],
    };
  });

  labels.sort((a, b) => b.damage - a.damage);

  const targetItems = targets.map((target) => {
    if (typeof target == "object" && Object.hasOwn(target, "Unknown")) {
      const hash = target.Unknown.toString(16).padStart(8, "0");

      return {
        rawValue: target,
        value: target.Unknown.toString(),
        label: t([`enemies.unknown.${hash}`, "enemies.unknown-type"], { id: hash }),
      };
    }

    return {
      rawValue: target,
      value: target.toString(),
      label: t([`enemies.${target}`, "enemies.unknown-type"]),
    };
  });

  return (
    <Box>
      <Text>
        <Box display="flex">
          <Box display="flex" flex={1}>
            <Button size="xs" variant="default" component={Link} to="/logs">
              {t("ui.back-btn")}
            </Button>
          </Box>
          <Flex display="flex" flex={1} justify={"flex-end"}>
            <Menu shadow="md" trigger="hover" openDelay={100} closeDelay={400}>
              <Menu.Target>
                <ActionIcon aria-label="Clipboard" variant="filled" color="light">
                  <ClipboardText size={16} />
                </ActionIcon>
              </Menu.Target>
              <Menu.Dropdown>
                <Menu.Item onClick={handleSimpleEncounterCopy}>{t("ui.copy-to-clipboard-simple")}</Menu.Item>
                <Menu.Item onClick={handleFullEncounterCopy}>{t("ui.copy-to-clipboard-full")}</Menu.Item>
                <Menu.Item onClick={exportDamageLogToFile}>{t("ui.export-damage-log")}</Menu.Item>
              </Menu.Dropdown>
            </Menu>
          </Flex>
        </Box>
      </Text>

      <Divider my="sm" />

      <Box>
        {questId && (
          <Text size="sm">
            {t("ui.logs.quest-name")}: {translateQuestId(questId)} ({toHash(questId)})
          </Text>
        )}
        <Text size="sm">
          {t("ui.logs.date")}: {epochToLocalTime(encounter.startTime)}
        </Text>
        <Text size="sm">
          {t("ui.logs.duration")}: {millisecondsToElapsedFormat(encounter.endTime - encounter.startTime)}
        </Text>
        {questTimer && (
          <Text size="sm">
            {t("ui.logs.quest-elapsed-time")}: {millisecondsToElapsedFormat(questTimer * 1000)}
          </Text>
        )}
        <Text size="sm">
          {t("ui.logs.total-damage")}: <NumberFormatter thousandSeparator value={encounter.totalDamage} />
        </Text>
      </Box>

      <Divider my="sm" />

      <Tabs defaultValue="overview" variant="outline">
        <Tabs.List>
          <Tabs.Tab value="overview">{t("ui.logs.overview")}</Tabs.Tab>
          <Tabs.Tab value="equipment" disabled={playerData.length === 0}>
            {t("ui.logs.equipment")}
          </Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="overview">
          <Box mt="md">
            <Stack>
              <MultiSelect
                data={targetItems}
                placeholder="All"
                clearable
                onChange={(value) => {
                  const targets = value
                    .map((v) => targetItems.find((t) => t.value === v)?.rawValue)
                    .filter((v) => v !== undefined) as EnemyType[];

                  setSelectedTargets(targets);
                }}
              />
              <MeterTable
                encounterState={encounter}
                sortType={sortType}
                sortDirection={sortDirection}
                setSortType={setSortType}
                setSortDirection={setSortDirection}
                partyData={playerData}
              />
              <Text size="sm">{t("ui.logs.damage-per-second")}</Text>
              <LineChart
                h={400}
                data={data}
                dataKey="timestamp"
                withDots={false}
                withLegend
                series={labels}
                valueFormatter={(value) => {
                  const [num, suffix] = humanizeNumbers(value);
                  return `${num}${suffix}`;
                }}
                tooltipProps={{
                  content: ({ label, payload }) => <ChartTooltip label={label} payload={payload} />,
                }}
              />
            </Stack>
          </Box>
        </Tabs.Panel>
        <Tabs.Panel value="equipment">
          <Group mt="20" gap="xs">
            <Table striped layout="fixed">
              <Table.Tbody>
                <Table.Tr>
                  {playerData.map((player) => {
                    return (
                      <Table.Td key={player.actorIndex} flex={1}>
                        <Text fw={700} size="xl">
                          {player.displayName} ({t(`characters.${player.characterType}`)})
                        </Text>
                      </Table.Td>
                    );
                  })}
                </Table.Tr>
                {Array.from(Array(12).keys()).map((sigilIndex) => (
                  <Table.Tr key={sigilIndex}>
                    {playerData.map((player) => {
                      const sigil = player.sigils[sigilIndex];

                      if (!sigil || sigil.sigilId === EMPTY_ID) {
                        return (
                          <Table.Td key={player.actorIndex}>
                            <Text size="xs" fw={300}>
                              ---
                            </Text>
                          </Table.Td>
                        );
                      }

                      return (
                        <Table.Td key={player.actorIndex}>
                          <Text size="xs" fw={700}>
                            {translateSigilId(sigil.sigilId)} (Lvl. {sigil.sigilLevel})
                          </Text>
                          <Text size="xs" fs="italic" fw={300}>
                            {translateTraitId(sigil.firstTraitId)}
                            {sigil.secondTraitId !== EMPTY_ID && ` / ${translateTraitId(sigil.secondTraitId)}`}
                          </Text>
                        </Table.Td>
                      );
                    })}
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Group>
        </Tabs.Panel>
      </Tabs>
    </Box>
  );
};
