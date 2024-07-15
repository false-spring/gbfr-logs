import { CharacterType, DeathEvent, EncounterState, EnemyType, PlayerData, SBAEvent } from "@/types";
import { create } from "zustand";

interface EncounterStore {
  encounterState: EncounterState | null;
  dpsChart: Record<number, number[]>;
  sbaChart: Record<number, number[]>;
  sbaEvents: SBAEvent[];
  deathEvents: DeathEvent[];
  chartLen: number;
  sbaChartLen: number;
  targets: EnemyType[];
  selectedTargets: EnemyType[];
  selectedPlayers: string[];
  selectedPlayerTypes: EnemyType[];
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean;
  setSelectedTargets: (targets: EnemyType[]) => void;
  setSelectedPlayers: (playerNames: string[]) => void;
  setSelectedPlayerTypes: (playerTypes: CharacterType[]) => void;
  loadFromResponse: (response: EncounterStateResponse) => void;
}

export interface EncounterStateResponse {
  encounterState: EncounterState;
  dpsChart: Record<number, number[]>;
  sbaChart: Record<number, number[]>;
  sbaEvents: SBAEvent[];
  deathEvents: DeathEvent[];
  chartLen: number;
  sbaChartLen: number;
  targets: EnemyType[];
  players: PlayerData[];
  questId: number | null;
  questTimer: number | null;
  questCompleted: boolean | null;
}

export const useEncounterStore = create<EncounterStore>((set) => ({
  encounterState: null,
  dpsChart: {},
  sbaChart: {},
  sbaEvents: [],
  deathEvents: [],
  chartLen: 0,
  sbaChartLen: 0,
  targets: [],
  selectedTargets: [],
  selectedPlayers: [],
  selectedPlayerTypes: [],
  players: [],
  questId: null,
  questTimer: null,
  questCompleted: false,
  setSelectedTargets: (targets: EnemyType[]) => set({ selectedTargets: targets }),
  setSelectedPlayers: (playerNames: string[]) => set({ selectedPlayers: playerNames }),
  setSelectedPlayerTypes: (playerTypes: CharacterType[]) => set({ selectedPlayerTypes: playerTypes }),
  loadFromResponse: (response: EncounterStateResponse) => {
    const filteredPlayers = response.players.filter((player) => player !== null);

    set({
      encounterState: response.encounterState,
      dpsChart: response.dpsChart,
      sbaChart: response.sbaChart,
      sbaEvents: response.sbaEvents,
      deathEvents: response.deathEvents,
      chartLen: response.chartLen,
      sbaChartLen: response.sbaChartLen,
      targets: response.targets,
      players: filteredPlayers,
      questId: response.questId,
      questTimer: response.questTimer,
      questCompleted: response.questCompleted || false,
    });
  },
}));
