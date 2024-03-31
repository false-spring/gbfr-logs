import { LineChart } from "@mantine/charts";
import {
  ActionIcon,
  Box,
  Button,
  Divider,
  Flex,
  Group,
  Menu,
  MultiSelect,
  NumberFormatter,
  Paper,
  Stack,
  Table,
  Tabs,
  Text,
} from "@mantine/core";
import { ClipboardText } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { t } from "i18next";
import { useCallback, useEffect, useState } from "react";
import toast from "react-hot-toast";
import { Link, useParams } from "react-router-dom";

import { Table as MeterTable } from "@/components/Table";
import { EncounterStateResponse, useEncounterStore } from "@/stores/useEncounterStore";
import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import type { ComputedPlayerState, EnemyType, Overmastery, PlayerData, SortDirection, SortType } from "@/types";
import {
  EMPTY_ID,
  PLAYER_COLORS,
  epochToLocalTime,
  exportFullEncounterToClipboard,
  exportScreenshotToClipboard,
  exportSimpleEncounterToClipboard,
  exportCharacterDataToClipboard,
  formatInPartyOrder,
  humanizeNumbers,
  millisecondsToElapsedFormat,
  toHash,
  toHashString,
  translateItemId,
  translateOvermasteryId,
  translateQuestId,
  translateSigilId,
  translateTraitId,
  translatedPlayerName,
} from "@/utils";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

type Label = { name: string; partySlotIndex: number; label?: string; color: string; strokeDasharray?: string }[];

const formatOvermastery = (overmastery: Overmastery): string => {
  const value = overmastery.value.toFixed(0);
  const translation = translateOvermasteryId(overmastery.id);
  const regularNumbers = [
    0x032a5217, 0x0781c7a2, 0x0b134a7f, 0x0cf5d0f3, 0x0db88f30, 0x0f25b474, 0x0febc993, 0x11023c6f, 0x124db819,
    0x1268b903, 0x13c9452a, 0x155c25c3, 0x1cc2f730, 0x1e2b3db5, 0x24499a25, 0x254a08d4, 0x2d6c03eb, 0x2ea457f3,
    0x303becc0, 0x3526fecb, 0x38f656e7, 0x394083bd, 0x3ac53494, 0x3ca4c8d5, 0x3d6600d9, 0x403be586, 0x409df671,
    0x427b5e26, 0x437c055d, 0x44f04a7a, 0x49089d4f, 0x4ab91ea7, 0x4c0cbd32, 0x4ce64874, 0x4e2513df, 0x52a207b5,
    0x5382923d, 0x53d358e0, 0x5767dd9f, 0x57bbc478, 0x59dce1e8, 0x5a51f0cb, 0x5a57dc07, 0x60835d4f, 0x60926b53,
    0x61d4efa0, 0x6564c02b, 0x66092bc7, 0x67bde89b, 0x6e4f2f5e, 0x6fb47781, 0x7125942e, 0x7cbbb4e0, 0x7ccf98c5,
    0x7e870ebe, 0x807e9e58, 0x829b8b5c, 0x834892b4, 0x85f0f318, 0x871d12cc, 0x874353d7, 0x8af65803, 0x8e66b68c,
    0x8fe7fb0a, 0x911d4f18, 0x91265f66, 0x93572974, 0x937efb96, 0x95567556, 0x9a0988df, 0x9a29aa64, 0x9b6f164c,
    0x9bfd4548, 0x9c6375cf, 0xa1dc63b3, 0xa257dac1, 0xa2bcf523, 0xa3460028, 0xa85b4af5, 0xaac23948, 0xab56bde3,
    0xaccbece1, 0xaf0d8b97, 0xb83aa115, 0xbbe7992a, 0xbd488071, 0xbe8c17d4, 0xbf44c20b, 0xc1360291, 0xc265b03b,
    0xc2d708c1, 0xc4925bd7, 0xc52d2245, 0xc5d68c62, 0xc6bdc7a6, 0xcb43ff8e, 0xcb63be55, 0xcb6bb434, 0xccef4492,
    0xcd5d6315, 0xcf24e1a2, 0xcf6b267a, 0xd51958d1, 0xda546dfe, 0xdcbd8423, 0xddc29837, 0xde6a367a, 0xdf2cab83,
    0xdf2eef09, 0xdfb00115, 0xe056ba80, 0xe7710898, 0xea5eaafc, 0xea99fa76, 0xee6100ca, 0xeefb4ade, 0xf004e9f2,
    0xf203bb15, 0xf2111b99, 0xf5514f81, 0xf80e3310, 0xfa230938, 0xfa9bcf64, 0xfb276afd, 0xfe71865d, 0x2676f9d2,
    0x2c1c933d, 0x3356dd03, 0x36f068fd, 0x3dae6494, 0x455d6a1c, 0x59fbb7d8, 0x6837e60c, 0x6cb38ef3, 0x7b05e679,
    0x7b498c32, 0x9bf7878a, 0xa3545ca1, 0xa85495ba, 0xa901e065, 0xc11fdfbd, 0xd5169339, 0xd63dd12b, 0xf5c314a0,
  ];

  let isRegularNumber = false;

  if (regularNumbers.includes(overmastery.id)) {
    isRegularNumber = true;
  }

  if (isRegularNumber) {
    return `${translation}: +${value}`;
  } else {
    return `${translation}: +${value}%`;
  }
};

const formatPlayerDisplayName = (player: PlayerData, showLevel: Boolean = true): string => {
  const displayName = player.displayName;
  const characterType = t(`characters:${player.characterType}`, `ui:characters.${player.characterType}`);

  if (showLevel) {
    if (displayName === "") {
      return `${characterType} Lvl. ${player.playerStats?.level || 1}`;
    } else {
      return `${displayName} (${characterType}) Lvl. ${player.playerStats?.level || 1}`;
    }
  }

  if (displayName === "") {
    return `${characterType}`;
  } else {
    return `${displayName} (${characterType})`;
  }
};

// Returns a string of stars based on the star level.
// ★★★☆☆☆ (3 stars)
// ★★★★★★ (6 stars)
const createWeaponStars = (starLevel: number): string => {
  return "★".repeat(starLevel) + "☆".repeat(6 - starLevel);
};

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
              {item.name === "party" ? t("ui.logs.damage-per-second") : item.name}
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
  const { color_1, color_2, color_3, color_4 } = useMeterSettingsStore(
    useShallow((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
    }))
  );
  const playerColors = [color_1, color_2, color_3, color_4, ...PLAYER_COLORS.slice(4)];

  const { t } = useTranslation();
  const { id } = useParams();

  const {
    encounter,
    dpsChart,
    sbaChart,
    sbaEvents,
    chartLen,
    sbaChartLen,
    targets,
    selectedTargets,
    questId,
    questTimer,
    questCompleted,
    playerData,
    setSelectedTargets,
    loadFromResponse,
  } = useEncounterStore((state) => ({
    encounter: state.encounterState,
    dpsChart: state.dpsChart,
    sbaChart: state.sbaChart,
    sbaEvents: state.sbaEvents,
    chartLen: state.chartLen,
    sbaChartLen: state.sbaChartLen,
    targets: state.targets,
    selectedTargets: state.selectedTargets,
    playerData: state.players,
    questId: state.questId,
    questTimer: state.questTimer,
    questCompleted: state.questCompleted,
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

  const handleCharacterDataCopy = useCallback((player) => {
    if (player) exportCharacterDataToClipboard(player);
  }, []);

  const handleSimpleEncounterCopy = useCallback(() => {
    if (encounter) exportSimpleEncounterToClipboard(sortType, sortDirection, encounter, playerData);
  }, [sortType, sortDirection, encounter]);

  const handleFullEncounterCopy = useCallback(() => {
    if (encounter) exportFullEncounterToClipboard(sortType, sortDirection, encounter, playerData);
  }, [sortType, sortDirection, encounter]);

  const handleScreenshotCopy = useCallback(() => {
    exportScreenshotToClipboard(".mantine-Tabs-root");
  }, []);

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
  const sbaData = [];

  const players = formatInPartyOrder(encounter.party);

  for (let i = 0; i < chartLen; i++) {
    const datapoint: {
      timestamp?: string;
      party?: number;
    } & { [key: string]: number } = {};

    const timestamp = i * (DPS_INTERVAL * 1000);

    datapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);
    datapoint["party"] = 0;

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

      const value = Math.round(averageValue / DPS_INTERVAL);
      datapoint[playerName] = value;
      datapoint["party"] += value;
    }

    data.push(datapoint);
  }

  for (let i = 0; i < sbaChartLen; i++) {
    const sbaDatapoint: {
      timestamp?: string;
    } & { [key: string]: number } = {};

    const timestamp = i * 1_000;

    sbaDatapoint["timestamp"] = millisecondsToElapsedFormat(timestamp);

    for (const playerIndex in sbaChart) {
      const player = players.find((p) => p.index === Number(playerIndex));
      const partySlotIndex = playerData.findIndex((partyMember) => partyMember?.actorIndex === player?.index);
      const playerName = translatedPlayerName(
        partySlotIndex,
        playerData[partySlotIndex],
        player as ComputedPlayerState
      );

      const value = sbaChart[playerIndex][i];
      sbaDatapoint[playerName] = value / 10.0;
    }

    sbaData.push(sbaDatapoint);
  }

  const labels: Label = players.map((player) => {
    const partySlotIndex = playerData.findIndex((partyMember) => partyMember?.actorIndex === player.index);
    const color = partySlotIndex !== -1 ? playerColors[partySlotIndex] : playerColors[player.partyIndex];

    return {
      name: translatedPlayerName(partySlotIndex, playerData[partySlotIndex], player),
      damage: player.totalDamage,
      partySlotIndex,
      color,
    };
  });

  const sbaLabels = labels.slice().filter((label) => label.partySlotIndex !== -1);

  labels.push({
    name: "party",
    partySlotIndex: -1,
    label: t("ui.logs.damage-per-second"),
    color: "grey",
    strokeDasharray: "2 2",
  });

  const targetItems = targets.map((target) => {
    if (typeof target == "object" && Object.hasOwn(target, "Unknown")) {
      const hash = target.Unknown.toString(16).padStart(8, "0");

      return {
        rawValue: target,
        value: target.Unknown.toString(),
        label: t([`enemies:${hash}.text`, `enemies.unknown.${hash}`, "enemies.unknown-type"], { id: hash }),
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
                <Menu.Item onClick={handleScreenshotCopy}>{t("ui.copy-screenshot-to-clipboard")}</Menu.Item>
                <Menu.Item onClick={exportDamageLogToFile}>{t("ui.export-damage-log")}</Menu.Item>
              </Menu.Dropdown>
            </Menu>
          </Flex>
        </Box>
      </Text>

      <Divider my="sm" />

      <Box>
        {questId && (
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.quest-name")}:
            </Text>
            <Text size="sm" ml={4}>
              {translateQuestId(questId)} ({toHash(questId)}){" "}
            </Text>
          </Box>
        )}
        {questId && (
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.quest-status")}:
            </Text>
            <Text size="sm" fs="italic" ml={4}>
              {questCompleted ? "✅" : "❌"}
            </Text>
          </Box>
        )}
        <Box display="flex">
          <Text size="sm" fw={800}>
            {t("ui.logs.date")}:
          </Text>
          <Text size="sm" fs="italic" ml={4}>
            {epochToLocalTime(encounter.startTime)}
          </Text>
        </Box>
        <Box display="flex">
          <Text size="sm" fw={800}>
            {t("ui.logs.duration")}:
          </Text>
          <Text size="sm" fs="italic" ml={4}>
            {millisecondsToElapsedFormat(encounter.endTime - encounter.startTime)}
          </Text>
        </Box>
        {questTimer && (
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.quest-elapsed-time")}:
            </Text>
            <Text size="sm" fs="italic" ml={4}>
              {millisecondsToElapsedFormat(questTimer * 1000)}
            </Text>
          </Box>
        )}
        <Box display="flex">
          <Text size="sm" fw={800}>
            {t("ui.logs.total-damage")}:
          </Text>
          <Text size="sm" fs="italic" ml={4}>
            <NumberFormatter thousandSeparator value={encounter.totalDamage} />
          </Text>
        </Box>
      </Box>

      <Divider my="sm" />

      <Tabs defaultValue="overview" variant="outline">
        <Tabs.List>
          <Tabs.Tab value="overview">{t("ui.logs.overview")}</Tabs.Tab>
          <Tabs.Tab value="sba">{t("ui.logs.sba-chart")}</Tabs.Tab>
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
        <Tabs.Panel value="sba">
          <Group mt="20" gap="xs">
            <Text size="sm">{t("ui.logs.sba-chart")}</Text>
            <LineChart
              h={400}
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
            />
            <Table striped layout="fixed">
              <Table.Tbody>
                {sbaEvents.map((payload, index) => {
                  const [timestamp, event] = payload;
                  const eventType = Object.keys(event)[0];

                  // @ts-expect-error: eventType is dynamic here.
                  const player = players.find((p) => p.index === event[eventType].actor_index);

                  const partySlotIndex = playerData.findIndex(
                    // @ts-expect-error: eventType is dynamic here.
                    (partyMember) => partyMember?.actorIndex === event[eventType].actor_index
                  );

                  const playerName = translatedPlayerName(
                    partySlotIndex,
                    playerData[partySlotIndex],
                    player as ComputedPlayerState
                  );

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
        </Tabs.Panel>
        <Tabs.Panel value="equipment">
          <Group mt="20" gap="xs">
            <Table striped layout="fixed">
              <Table.Tbody>
                <Table.Tr>
                  {playerData.map((player) => {
                    return (
                      <Table.Td key={player.actorIndex} flex={1}>
                        <Flex direction="row" wrap="nowrap" align="center">    
                          <Text fw={700} size="xl" mr="5">
                            {formatPlayerDisplayName(player, false)}
                          </Text>
                          <ActionIcon aria-label="Clipboard" variant="filled" color="light" onClick={() => handleCharacterDataCopy(player)}>
                            <ClipboardText size={16} />
                          </ActionIcon>
                        </Flex>
                      </Table.Td>
                    );
                  })}
                </Table.Tr>
                <Table.Tr>
                  {playerData.map((player) => {
                    return (
                      <Table.Td key={player.actorIndex}>
                        <Text size="xs" fw={700}>
                          {t("ui.player-stats")}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.level")}: {player.playerStats?.level || 1}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.total-hp")}: {player.playerStats?.totalHp || 1}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.total-attack")}: {player.playerStats?.totalAttack || 1}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.critical-rate")}: {(player.playerStats?.criticalRate || 0).toFixed(0)}%
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.stun-power")}: {((player.playerStats?.stunPower || 0) * 10).toFixed(0)}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t("ui.stats.total-power")}: {player.playerStats?.totalPower || 1}
                        </Text>
                      </Table.Td>
                    );
                  })}
                </Table.Tr>
                <Table.Tr>
                  {playerData.map((player) => {
                    const overmasteries = player.overmasteryInfo?.overmasteries || [];

                    return (
                      <Table.Td key={player.actorIndex}>
                        <Text size="xs" fw={700}>
                          {t("ui.player-overmasteries")}
                        </Text>
                        {Array.from(Array(4).keys()).map((overmasteryIndex) => {
                          const overmastery = overmasteries[overmasteryIndex];

                          return (
                            <Placeholder key={overmasteryIndex} empty={!overmastery || overmastery.value === 0}>
                              <Text size="xs" fs="italic" fw={300}>
                                {formatOvermastery(overmastery)}
                              </Text>
                            </Placeholder>
                          );
                        })}
                      </Table.Td>
                    );
                  })}
                </Table.Tr>
                <Table.Tr>
                  {playerData.map((player) => {
                    return (
                      <Table.Td key={player.actorIndex}>
                        <Text size="xs" fw={700}>
                          {t("ui.weapon")}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {createWeaponStars(player.weaponInfo?.starLevel || 0)}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          {t([`weapons:${toHashString(player.weaponInfo?.weaponId)}.text`, "unknown"])} +
                          {player.weaponInfo?.plusMarks}
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          Awakening {player.weaponInfo?.awakeningLevel || 0}/10
                        </Text>
                        <Text size="xs" fs="italic" fw={300}>
                          Lvl {player.weaponInfo?.weaponLevel || 0} / ATK {player.weaponInfo?.weaponAttack || 0} / HP{" "}
                          {player.weaponInfo?.weaponHp || 0}
                        </Text>
                        <Text size="xs" fw={700}>
                          {translateItemId(player.weaponInfo?.wrightstoneId || EMPTY_ID)}
                        </Text>
                        <Placeholder empty={!player.weaponInfo?.trait1Id || player.weaponInfo?.trait1Level == 0}>
                          <Text size="xs" fs="italic" fw={300}>
                            - {translateTraitId(player.weaponInfo?.trait1Id || EMPTY_ID)} (Lvl.{" "}
                            {player.weaponInfo?.trait1Level})
                          </Text>
                        </Placeholder>
                        <Placeholder empty={!player.weaponInfo?.trait2Id || player.weaponInfo?.trait2Level == 0}>
                          <Text size="xs" fs="italic" fw={300}>
                            - {translateTraitId(player.weaponInfo?.trait2Id || EMPTY_ID)} (Lvl.{" "}
                            {player.weaponInfo?.trait2Level})
                          </Text>
                        </Placeholder>
                        <Placeholder empty={!player.weaponInfo?.trait3Id || player.weaponInfo?.trait3Level == 0}>
                          <Text size="xs" fs="italic" fw={300}>
                            - {translateTraitId(player.weaponInfo?.trait3Id || EMPTY_ID)} (Lvl.{" "}
                            {player.weaponInfo?.trait3Level})
                          </Text>
                        </Placeholder>
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
                            <Placeholder empty />
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

function Placeholder({ empty, children }: { empty: boolean; children?: React.ReactNode }) {
  return empty ? (
    <Text size="xs" fw={300}>
      ---
    </Text>
  ) : (
    children
  );
}
