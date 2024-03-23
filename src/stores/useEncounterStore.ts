import { EncounterState, EnemyType, PlayerData } from "@/types";
import { create } from "zustand";

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
