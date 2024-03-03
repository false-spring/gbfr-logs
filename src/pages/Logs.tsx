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
  Slider,
  Select,
  Stack,
  Space,
  Center,
  NumberFormatter,
  Paper,
} from "@mantine/core";
import { LineChart } from "@mantine/charts";
import { useDisclosure } from "@mantine/hooks";
import { Gear, House } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { useEffect } from "react";
import { Link, Outlet, useParams } from "react-router-dom";
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { t } from "i18next";
import toast, { Toaster } from "react-hot-toast";

import { Table as MeterTable } from "../components/Table";
import {
  PLAYER_COLORS,
  epochToLocalTime,
  formatInPartyOrder,
  humanizeNumbers,
  millisecondsToElapsedFormat,
  translatedPlayerName,
} from "../utils";
import { ComputedPlayerData, EncounterState } from "../types";

interface SearchResult {
  logs: Log[];
  page: number;
  pageCount: number;
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
  setSearchResult: (result: SearchResult) => void;
  setCurrentPage: (page: number) => void;
}

const useLogIndexStore = create<LogIndexState>((set) => ({
  currentPage: 1,
  searchResult: { logs: [], page: 1, pageCount: 0 },
  setCurrentPage: (page: number) => set({ currentPage: page }),
  setSearchResult: (result) => set({ searchResult: result }),
}));

interface EncounterStore {
  encounterState: EncounterState | null;
  dpsChart: Record<number, number[]>;
  chartLen: number;
  loadFromResponse: (response: EncounterStateResponse) => void;
}

interface EncounterStateResponse {
  encounterState: EncounterState;
  dpsChart: Record<number, number[]>;
  chartLen: number;
}

const useEncounterStore = create<EncounterStore>((set) => ({
  encounterState: null,
  dpsChart: {},
  chartLen: 0,
  loadFromResponse: (response: EncounterStateResponse) =>
    set({
      encounterState: response.encounterState,
      dpsChart: response.dpsChart,
      chartLen: response.chartLen,
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
  const encounter = useEncounterStore((state) => state.encounterState);
  const dpsChart = useEncounterStore((state) => state.dpsChart);
  const chartLen = useEncounterStore((state) => state.chartLen);
  const loadFromResponse = useEncounterStore((state) => state.loadFromResponse);

  useEffect(() => {
    invoke("fetch_encounter_state", { id: Number(id) })
      .then((result) => {
        loadFromResponse(result as EncounterStateResponse);
      })
      .catch((e) => {
        toast.error(`Failed to fetch encounter state: ${e}`);
      });
  }, [id]);

  if (!encounter) {
    return (
      <Box>
        <Text>
          <Link to="/logs">Back</Link>
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

  return (
    <Box>
      <Text>
        <Button size="xs" variant="default" component={Link} to="/logs">
          Back
        </Button>
      </Text>
      <Divider my="sm" />
      <Stack>
        <Box>
          <Text size="sm">Date: {epochToLocalTime(encounter.endTime)}</Text>
          <Text size="sm">Duration: {millisecondsToElapsedFormat(encounter.endTime - encounter.startTime)}</Text>
          <Text size="sm">
            Total Damage: <NumberFormatter thousandSeparator value={encounter.totalDamage} />
          </Text>
        </Box>
        <MeterTable encounterState={encounter} />
        <Text size="sm">Damage Per Second</Text>
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
  const currentPage = useLogIndexStore((state) => state.currentPage);
  const setCurrentPage = useLogIndexStore((state) => state.setCurrentPage);
  const searchResult = useLogIndexStore((state) => state.searchResult);
  const setSearchResult = useLogIndexStore((state) => state.setSearchResult);

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

    return (
      <Table.Tr key={log.id}>
        <Table.Td>
          <Text size="sm">{epochToLocalTime(log.time)}</Text>
        </Table.Td>
        <Table.Td>
          {millisecondsToElapsedFormat(log.duration)} - {names}
        </Table.Td>
        <Table.Td>
          <Button size="xs" variant="default" component={Link} to={`/logs/${log.id}`}>
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
              <Table.Th>Date</Table.Th>
              <Table.Th>Name</Table.Th>
              <Table.Th></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody></Table.Tbody>
        </Table>
        <Space h="sm" />
        <Center>
          <Text>No logs recorded yet.</Text>
        </Center>
        <Divider my="sm" />
        <Pagination total={1} disabled />
      </Box>
    );
  } else {
    return (
      <Box>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Date</Table.Th>
              <Table.Th>Name</Table.Th>
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
  return (
    <Box>
      <Fieldset legend="Meter Settings">
        <Stack>
          <Text size="sm">Nothing here yet.</Text>
        </Stack>
      </Fieldset>
    </Box>
  );

  // @TODO(false): Implement user config settings.
  return (
    <Box>
      <Fieldset legend="Meter Settings">
        <Stack>
          <Text size="sm">Background Opacity</Text>
          <Slider defaultValue={0} label={(value) => `${value}%`} />
          <Text size="sm">Copy-to-clipboard Text Format</Text>
          <Select data={["Normal", "Compact"]} defaultValue="Normal" allowDeselect={false} />
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
