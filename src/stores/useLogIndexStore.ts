import { invoke } from "@tauri-apps/api";
import toast from "react-hot-toast";
import { create } from "zustand";

import { EnemyType } from "../types";

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
