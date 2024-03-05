import "./Logs.css";

import {
  AppShell,
  Box,
  Burger,
  Button,
  Divider,
  Group,
  NavLink,
  Table,
  Text,
  Pagination,
  Fieldset,
  Select,
  Stack,
  Space,
  Center,
  NumberFormatter,
  Paper,
  Checkbox,
  MultiSelect,
  Menu,
  ActionIcon,
  Flex,
} from "@mantine/core";
import { LineChart } from "@mantine/charts";
import { useDisclosure } from "@mantine/hooks";
import { ClipboardText, Gear, House } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { useCallback, useEffect } from "react";
import { Link, Outlet, useParams } from "react-router-dom";
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { t } from "i18next";
import toast, { Toaster } from "react-hot-toast";

import { Table as MeterTable } from "../components/Table";
import {
  PLAYER_COLORS,
  epochToLocalTime,
  exportFullEncounterToClipboard,
  exportSimpleEncounterToClipboard,
  formatInPartyOrder,
  humanizeNumbers,
  millisecondsToElapsedFormat,
  translatedPlayerName,
} from "../utils";
import { ComputedPlayerData, EncounterState, EnemyType } from "../types";
import { useTranslation } from "react-i18next";
import { SUPPORTED_LANGUAGES } from "../i18n";

interface SearchResult {
  logs: Log[];
  page: number;
  pageCount: number;
  logCount: number;
}

interface Log {
  id: number;
  name: string;
  time: number;
  duration: number;
}

interface LogIndexState {
  currentPage: number;
  searchResult: SearchResult;
  selectedLogIds: number[];
  setSearchResult: (result: SearchResult) => void;
  setCurrentPage: (page: number) => void;
  setSelectedLogIds: (ids: number[]) => void;
  deleteSelectedLogs: () => void;
}

const useLogIndexStore = create<LogIndexState>((set, get) => ({
  currentPage: 1,
  searchResult: { logs: [], page: 1, pageCount: 0, logCount: 0 },
  selectedLogIds: [],
  setCurrentPage: (page: number) => set({ currentPage: page }),
  setSearchResult: (result) => set({ searchResult: result }),
  setSelectedLogIds: (ids) => set({ selectedLogIds: ids }),
  deleteSelectedLogs: async () => {
    const { setSearchResult, selectedLogIds: ids } = get();

    try {
      await invoke("delete_logs", { ids });
      set({ currentPage: 1, selectedLogIds: [], searchResult: { logs: [], page: 1, pageCount: 0, logCount: 0 } });
      toast.success("Logs deleted successfully.");
      const result = await invoke("fetch_logs");
      setSearchResult(result as SearchResult);
    } catch (e) {
      toast.error(`Failed to delete logs: ${e}`);
    }
  },
}));

interface EncounterStore {
  encounterState: EncounterState | null;
  dpsChart: Record<number, number[]>;
  chartLen: number;
  targets: EnemyType[];
  selectedTargets: EnemyType[];
  setSelectedTargets: (targets: EnemyType[]) => void;
  loadFromResponse: (response: EncounterStateResponse) => void;
}

interface EncounterStateResponse {
  encounterState: EncounterState;
  dpsChart: Record<number, number[]>;
  chartLen: number;
  targets: EnemyType[];
}

const useEncounterStore = create<EncounterStore>((set) => ({
  encounterState: null,
  dpsChart: {},
  chartLen: 0,
  targets: [],
  selectedTargets: [],
  setSelectedTargets: (targets: EnemyType[]) => set({ selectedTargets: targets }),
  loadFromResponse: (response: EncounterStateResponse) =>
    set({
      encounterState: response.encounterState,
      dpsChart: response.dpsChart,
      chartLen: response.chartLen,
      targets: response.targets,
    }),
}));

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

const LogViewPage = () => {
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

const LogIndexPage = () => {
  const { t } = useTranslation();
  const {
    currentPage,
    setCurrentPage,
    searchResult,
    setSearchResult,
    selectedLogIds,
    setSelectedLogIds,
    deleteSelectedLogs,
  } = useLogIndexStore((state) => ({
    currentPage: state.currentPage,
    setCurrentPage: state.setCurrentPage,
    searchResult: state.searchResult,
    setSearchResult: state.setSearchResult,
    selectedLogIds: state.selectedLogIds,
    setSelectedLogIds: state.setSelectedLogIds,
    deleteSelectedLogs: state.deleteSelectedLogs,
  }));

  const { setSelectedTargets } = useEncounterStore((state) => ({
    setSelectedTargets: state.setSelectedTargets,
  }));

  useEffect(() => {
    invoke("fetch_logs").then((result) => {
      setSearchResult(result as SearchResult);
    });
  }, []);

  useEffect(() => {
    const encounterSavedListener = listen("encounter-saved", () => {
      invoke("fetch_logs", { page: currentPage }).then((result) => {
        setSearchResult(result as SearchResult);
      });
    });

    return () => {
      encounterSavedListener.then((f) => f());
    };
  }, [currentPage]);

  const handleSetPage = (page: number) => {
    setCurrentPage(page);
    invoke("fetch_logs", { page }).then((result) => {
      setSearchResult(result as SearchResult);
    });
  };

  const rows = searchResult.logs.map((log) => {
    const names = log.name
      .split(", ")
      .map((name) => t(`characters.${name}`))
      .join(", ");

    const resetSelectedTargets = () => {
      setSelectedTargets([]);
    };

    return (
      <Table.Tr key={log.id}>
        <Table.Td>
          <Checkbox
            aria-label="Select row"
            checked={selectedLogIds.includes(log.id)}
            onChange={(event) =>
              setSelectedLogIds(
                event.currentTarget.checked ? [...selectedLogIds, log.id] : selectedLogIds.filter((id) => id !== log.id)
              )
            }
          />
        </Table.Td>
        <Table.Td>
          <Text size="sm">{epochToLocalTime(log.time)}</Text>
        </Table.Td>
        <Table.Td>
          {millisecondsToElapsedFormat(log.duration)} - {names}
        </Table.Td>
        <Table.Td>
          <Button size="xs" variant="default" component={Link} to={`/logs/${log.id}`} onClick={resetSelectedTargets}>
            View
          </Button>
        </Table.Td>
      </Table.Tr>
    );
  });

  if (searchResult.logs.length === 0) {
    return (
      <Box>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>{t("ui.logs.date")}</Table.Th>
              <Table.Th>{t("ui.logs.name")}</Table.Th>
              <Table.Th></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody></Table.Tbody>
        </Table>
        <Space h="sm" />
        <Center>
          <Text>{t("ui.logs.saved-count", { count: 0 })}</Text>
        </Center>
        <Divider my="sm" />
        <Pagination total={1} disabled />
      </Box>
    );
  } else {
    return (
      <Box>
        <Group>
          <Box style={{ display: "flex" }}>
            <Text>{t("ui.logs.saved-count", { count: searchResult.logCount })}</Text>
          </Box>
          <Box style={{ display: "flex", flexDirection: "row-reverse", flex: 1 }}>
            <Button size="xs" variant="default" onClick={deleteSelectedLogs} disabled={selectedLogIds.length === 0}>
              {t("ui.logs.delete-selected-btn")}
            </Button>
          </Box>
        </Group>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th />
              <Table.Th>{t("ui.logs.date")}</Table.Th>
              <Table.Th>{t("ui.logs.name")}</Table.Th>
              <Table.Th></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>{rows}</Table.Tbody>
        </Table>
        <Divider my="sm" />
        <Pagination total={searchResult.pageCount} value={currentPage} onChange={handleSetPage} />
      </Box>
    );
  }
};

const SettingsPage = () => {
  const { t, i18n } = useTranslation();
  const languages = Object.keys(SUPPORTED_LANGUAGES).map((key) => ({ value: key, label: SUPPORTED_LANGUAGES[key] }));

  const handleLanguageChange = (language: string | null) => {
    i18n.changeLanguage(language as string);
  };

  return (
    <Box>
      <Fieldset legend={t("ui.meter-settings")}>
        <Stack>
          <Text size="sm">{t("ui.language")}</Text>
          <Select data={languages} defaultValue={i18n.language} allowDeselect={false} onChange={handleLanguageChange} />
        </Stack>
      </Fieldset>
    </Box>
  );
};

const Layout = () => {
  const [mobileOpened, { toggle: toggleMobile }] = useDisclosure();
  const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(true);

  return (
    <div className="log-window">
      <AppShell
        header={{ height: 50 }}
        navbar={{
          width: 300,
          breakpoint: "sm",
          collapsed: { mobile: !mobileOpened, desktop: !desktopOpened },
        }}
        padding="sm"
      >
        <AppShell.Header>
          <Group h="100%" px="sm">
            <Burger opened={mobileOpened} onClick={toggleMobile} hiddenFrom="sm" size="sm" />
            <Burger opened={desktopOpened} onClick={toggleDesktop} visibleFrom="sm" size="sm" />
            <Text>GBFR Logs</Text>
          </Group>
        </AppShell.Header>
        <AppShell.Navbar p="sm">
          <AppShell.Section grow>
            <NavLink label="Logs" leftSection={<House size="1rem" />} component={Link} to="/logs" />
          </AppShell.Section>
          <AppShell.Section>
            <NavLink label="Settings" leftSection={<Gear size="1rem" />} component={Link} to="/logs/settings" />
          </AppShell.Section>
        </AppShell.Navbar>
        <AppShell.Main>
          <Outlet />
        </AppShell.Main>
      </AppShell>
      <Toaster
        position="bottom-center"
        toastOptions={{
          style: {
            borderRadius: "10px",
            backgroundColor: "#252525",
            color: "#fff",
            fontSize: "14px",
          },
        }}
      />
    </div>
  );
};

export { LogIndexPage, LogViewPage, SettingsPage };

export default Layout;
