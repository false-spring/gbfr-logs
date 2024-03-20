import "./Logs.css";

import {
  AppShell,
  Box,
  Burger,
  Group,
  NavLink,
  Text,
  Fieldset,
  Select,
  Stack,
  ColorInput,
  Slider,
  Checkbox,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Gear, House } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { Link, Outlet } from "react-router-dom";
import { create } from "zustand";
import toast, { Toaster } from "react-hot-toast";
import { useTranslation } from "react-i18next";

import { EncounterState, EnemyType, PlayerData } from "../types";
import { SUPPORTED_LANGUAGES } from "../i18n";
import { useMeterSettingsStore } from "../Store";

export interface SearchResult {
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
  version: number;
  primaryTarget: EnemyType | null;
  p1Name: string | null;
  p1Type: string | null;
  p2Name: string | null;
  p2Type: string | null;
  p3Name: string | null;
  p3Type: string | null;
  p4Name: string | null;
  p4Type: string | null;
  questId: number | null;
  questElapsedTime: number | null;
  questCompleted: boolean;
}

interface LogIndexState {
  currentPage: number;
  searchResult: SearchResult;
  selectedLogIds: number[];
  setSearchResult: (result: SearchResult) => void;
  setCurrentPage: (page: number) => void;
  setSelectedLogIds: (ids: number[]) => void;
  deleteSelectedLogs: () => void;
  deleteAllLogs: () => void;
}

export const useLogIndexStore = create<LogIndexState>((set, get) => ({
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
  deleteAllLogs: async () => {
    const { setSearchResult } = get();

    try {
      await invoke("delete_all_logs");
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
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean;
  setSelectedTargets: (targets: EnemyType[]) => void;
  loadFromResponse: (response: EncounterStateResponse) => void;
}

export interface EncounterStateResponse {
  encounterState: EncounterState;
  dpsChart: Record<number, number[]>;
  chartLen: number;
  targets: EnemyType[];
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean | null;
}

export const useEncounterStore = create<EncounterStore>((set) => ({
  encounterState: null,
  dpsChart: {},
  chartLen: 0,
  targets: [],
  selectedTargets: [],
  players: [],
  questId: null,
  questTimer: null,
  questCompleted: false,
  setSelectedTargets: (targets: EnemyType[]) => set({ selectedTargets: targets }),
  loadFromResponse: (response: EncounterStateResponse) => {
    const filteredPlayers = response.players.filter((player) => player !== null);

    set({
      encounterState: response.encounterState,
      dpsChart: response.dpsChart,
      chartLen: response.chartLen,
      targets: response.targets,
      players: filteredPlayers,
      questId: response.questId,
      questTimer: response.questTimer,
      questCompleted: response.questCompleted || false,
    });
  },
}));

const SettingsPage = () => {
  const { color_1, color_2, color_3, color_4, transparency, show_display_names, setMeterSettings } =
    useMeterSettingsStore((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
      transparency: state.transparency,
      show_display_names: state.show_display_names,
      setMeterSettings: state.set,
    }));

  const { t, i18n } = useTranslation();

  const languages = Object.keys(SUPPORTED_LANGUAGES).map((key) => ({ value: key, label: SUPPORTED_LANGUAGES[key] }));

  const handleLanguageChange = (language: string | null) => {
    i18n.changeLanguage(language as string);
  };

  return (
    <Box>
      <Fieldset legend={t("ui.meter-settings")}>
        <Stack>
          <Select
            label={t("ui.language")}
            data={languages}
            defaultValue={i18n.language}
            allowDeselect={false}
            onChange={handleLanguageChange}
          />
          <ColorInput
            defaultValue={color_1}
            onChangeEnd={(value) => setMeterSettings({ color_1: value })}
            withEyeDropper={false}
            label={t("ui.player-1-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_2}
            onChangeEnd={(value) => setMeterSettings({ color_2: value })}
            withEyeDropper={false}
            label={t("ui.player-2-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_3}
            onChangeEnd={(value) => setMeterSettings({ color_3: value })}
            withEyeDropper={false}
            label={t("ui.player-3-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_4}
            onChangeEnd={(value) => setMeterSettings({ color_4: value })}
            withEyeDropper={false}
            label={t("ui.player-4-color")}
            placeholder="Color"
          />
          <Text size="sm">{t("ui.meter-transparency")}</Text>
          <Slider
            min={0}
            max={1}
            step={0.005}
            defaultValue={transparency}
            onChangeEnd={(value) => setMeterSettings({ transparency: value })}
          />
          <Checkbox
            label={t("ui.show-player-names")}
            checked={show_display_names}
            onChange={(event) => setMeterSettings({ show_display_names: event.currentTarget.checked })}
          />
        </Stack>
      </Fieldset>
    </Box>
  );
};

const Layout = () => {
  const [mobileOpened, { toggle: toggleMobile }] = useDisclosure();
  const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(false);

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

export { SettingsPage };

export default Layout;
