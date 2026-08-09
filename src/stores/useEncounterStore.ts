import {
  EncounterState,
  LinkTimeWindow,
  PlayerData,
  SBAEvent,
  StatusIntervals,
  StatusPeakStacks,
  StatusSources,
  StatusStackSeries,
  StatusValueIsFraction,
  StatusValueSeries,
} from "@/types";
import { invoke } from "@tauri-apps/api";
import { create } from "zustand";

interface EncounterStore {
  encounterState: EncounterState | null;
  encounterStatesByEnemy: Record<number, EncounterState>;
  dpsChartByEnemy: Record<number, Record<number, number[]>>;
  stunChartByEnemy: Record<number, Record<number, number[]>>;
  stunResetByEnemy: Record<number, number[]>;
  sbaChart: Record<number, number[]>;
  sbaEvents: SBAEvent[];
  healProvidedChart: Record<number, number[]>;
  healReceivedChart: Record<number, number[]>;
  damageTakenChart: Record<number, number[]>;
  deaths: Record<number, number[]>;
  miscChartLen: number;
  linkTimeWindows: LinkTimeWindow[];
  confluxAreaClears: number[];
  confluxBossClears: number[];
  sbaWindows: LinkTimeWindow[];
  breakWindows: LinkTimeWindow[];
  statusIntervals: StatusIntervals;
  statusPeakStacks: StatusPeakStacks;
  statusStackSeries: StatusStackSeries;
  statusValueSeries: StatusValueSeries;
  statusValueIsFraction: StatusValueIsFraction;
  statusSources: StatusSources;
  chartLen: number;
  sbaChartLen: number;
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean;
  loadFromResponse: (response: EncounterStateResponse) => void;
  fetchEnemyState: (id: number, enemyIndex: number) => Promise<void>;
}

export interface EncounterStateResponse {
  encounterState: EncounterState;
  dpsChartByEnemy: Record<number, Record<number, number[]>>;
  stunChartByEnemy: Record<number, Record<number, number[]>>;
  stunResetByEnemy: Record<number, number[]>;
  sbaChart: Record<number, number[]>;
  sbaEvents: SBAEvent[];
  healProvidedChart: Record<number, number[]>;
  healReceivedChart: Record<number, number[]>;
  damageTakenChart: Record<number, number[]>;
  deaths: Record<number, number[]>;
  miscChartLen: number;
  linkTimeWindows: LinkTimeWindow[];
  confluxAreaClears: number[];
  confluxBossClears: number[];
  sbaWindows: LinkTimeWindow[];
  breakWindows: LinkTimeWindow[];
  statusIntervals: StatusIntervals;
  statusPeakStacks: StatusPeakStacks;
  statusStackSeries: StatusStackSeries;
  statusValueSeries: StatusValueSeries;
  statusValueIsFraction: StatusValueIsFraction;
  statusSources: StatusSources;
  chartLen: number;
  sbaChartLen: number;
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean | null;
}

export const useEncounterStore = create<EncounterStore>((set, get) => ({
  encounterState: null,
  encounterStatesByEnemy: {},
  dpsChartByEnemy: {},
  stunChartByEnemy: {},
  stunResetByEnemy: {},
  sbaChart: {},
  sbaEvents: [],
  healProvidedChart: {},
  healReceivedChart: {},
  damageTakenChart: {},
  deaths: {},
  miscChartLen: 0,
  linkTimeWindows: [],
  confluxAreaClears: [],
  confluxBossClears: [],
  sbaWindows: [],
  breakWindows: [],
  statusIntervals: {},
  statusPeakStacks: {},
  statusStackSeries: {},
  statusValueSeries: {},
  statusValueIsFraction: {},
  statusSources: {},
  chartLen: 0,
  sbaChartLen: 0,
  players: [],
  questId: null,
  questTimer: null,
  questCompleted: false,
  loadFromResponse: (response: EncounterStateResponse) => {
    const filteredPlayers = response.players.filter((player) => player !== null);

    set({
      encounterState: response.encounterState,
      encounterStatesByEnemy: {},
      dpsChartByEnemy: response.dpsChartByEnemy ?? {},
      stunChartByEnemy: response.stunChartByEnemy ?? {},
      stunResetByEnemy: response.stunResetByEnemy ?? {},
      sbaChart: response.sbaChart,
      sbaEvents: response.sbaEvents,
      healProvidedChart: response.healProvidedChart ?? {},
      healReceivedChart: response.healReceivedChart ?? {},
      damageTakenChart: response.damageTakenChart ?? {},
      deaths: response.deaths ?? {},
      miscChartLen: response.miscChartLen ?? 0,
      linkTimeWindows: response.linkTimeWindows ?? [],
      confluxAreaClears: response.confluxAreaClears ?? [],
      confluxBossClears: response.confluxBossClears ?? [],
      sbaWindows: response.sbaWindows ?? [],
      breakWindows: response.breakWindows ?? [],
      statusIntervals: response.statusIntervals ?? {},
      statusPeakStacks: response.statusPeakStacks ?? {},
      statusStackSeries: response.statusStackSeries ?? {},
      statusValueSeries: response.statusValueSeries ?? {},
      statusValueIsFraction: response.statusValueIsFraction ?? {},
      statusSources: response.statusSources ?? {},
      chartLen: response.chartLen,
      sbaChartLen: response.sbaChartLen,
      players: filteredPlayers,
      questId: response.questId,
      questTimer: response.questTimer,
      questCompleted: response.questCompleted || false,
    });
  },
  fetchEnemyState: async (id: number, enemyIndex: number) => {
    if (get().encounterStatesByEnemy[enemyIndex]) return;
    try {
      const state = await invoke("fetch_enemy_encounter_state", { id, enemyIndex });
      set((prev) => ({
        encounterStatesByEnemy: { ...prev.encounterStatesByEnemy, [enemyIndex]: state as EncounterState },
      }));
    } catch (e) {
      console.error(`Failed to fetch enemy encounter state: ${e}`);
    }
  },
}));
