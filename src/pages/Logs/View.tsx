import {
    Box,
    Button,
    Divider,
    Text,
    Stack,
    NumberFormatter,
    Paper,
    MultiSelect,
    Menu,
    ActionIcon,
    Flex,
} from "@mantine/core";
import { LineChart } from "@mantine/charts";

import {
    PLAYER_COLORS,
    epochToLocalTime,
    exportFullEncounterToClipboard,
    exportSimpleEncounterToClipboard,
    formatInPartyOrder,
    humanizeNumbers,
    millisecondsToElapsedFormat,
    translatedPlayerName,
} from "../../utils";

import { useCallback, useEffect } from "react";
import { Link, useParams } from "react-router-dom";
import { useEncounterStore, EncounterStateResponse } from "../Logs";
import { invoke } from "@tauri-apps/api";
import toast, { Toaster } from "react-hot-toast";
import { ComputedPlayerData, EnemyType } from "../../types";
import { ClipboardText } from "@phosphor-icons/react";
import { t } from "i18next";
import { Table as MeterTable } from "../../components/Table";


interface ChartTooltipProps {
    label: string;
    payload: Record<string, any>[] | undefined; // eslint-disable-line
  }
  
  const ChartTooltip = ({ label, payload }: ChartTooltipProps) => {
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
  
  const DPS_INTERVAL = 5;
  
  const ViewPage = () => {
    const { id } = useParams();
    const { encounter, dpsChart, chartLen, targets, selectedTargets, setSelectedTargets, loadFromResponse } =
      useEncounterStore((state) => ({
        encounter: state.encounterState,
        dpsChart: state.dpsChart,
        chartLen: state.chartLen,
        targets: state.targets,
        selectedTargets: state.selectedTargets,
        setSelectedTargets: state.setSelectedTargets,
        loadFromResponse: state.loadFromResponse,
      }));
  
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
      if (encounter) exportSimpleEncounterToClipboard(encounter);
    }, [encounter]);
  
    const handleFullEncounterCopy = useCallback(() => {
      if (encounter) exportFullEncounterToClipboard(encounter);
    }, [encounter]);
  
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
        const playerName = translatedPlayerName(player as ComputedPlayerData);
  
        const lastFiveValues = dpsChart[playerIndex].slice(i - 5, i);
        const totalLastFiveValues = lastFiveValues.reduce((a, b) => a + b, 0);
        const currentValue = dpsChart[playerIndex][i] || 0;
        const averageValue = (totalLastFiveValues + currentValue) / (lastFiveValues.length + 1);
  
        datapoint[playerName] = Math.round(averageValue / DPS_INTERVAL);
      }
  
      data.push(datapoint);
    }
  
    const labels = players.map((player) => {
      return {
        name: translatedPlayerName(player),
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
                  <Menu.Item onClick={handleSimpleEncounterCopy}>
                    <Text size="xs">{t("ui.copy-to-clipboard-simple")}</Text>
                  </Menu.Item>
                  <Menu.Item onClick={handleFullEncounterCopy}>{t("ui.copy-to-clipboard-full")}</Menu.Item>
                  <Menu.Item onClick={exportDamageLogToFile}>{t("ui.export-damage-log")}</Menu.Item>
                </Menu.Dropdown>
              </Menu>
            </Flex>
          </Box>
        </Text>
        <Divider my="sm" />
        <Stack>
          <Box>
            <Text size="sm">
              {t("ui.logs.date")}: {epochToLocalTime(encounter.startTime)}
            </Text>
            <Text size="sm">
              {t("ui.logs.duration")}: {millisecondsToElapsedFormat(encounter.endTime - encounter.startTime)}
            </Text>
            <Text size="sm">
              {t("ui.logs.total-damage")}: <NumberFormatter thousandSeparator value={encounter.totalDamage} />
            </Text>
          </Box>
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
          <MeterTable encounterState={encounter} />
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
    );
  };
  
export { ViewPage };  