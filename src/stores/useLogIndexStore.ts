import { invoke } from "@tauri-apps/api";
import toast from "react-hot-toast";
import { create } from "zustand";

import { Log } from "@/types";

export type SearchResult = {
  logs: Log[];
  page: number;
  pageCount: number;
  logCount: number;
  enemyIds: number[];
  questIds: number[];
};

const DEFAULT_SEARCH_RESULT = {
  logs: [],
  page: 1,
  pageCount: 0,
  logCount: 0,
  enemyIds: [],
  questIds: [],
};

type LogIndexState = {
  currentPage: number;
  searchResult: SearchResult;
  selectedLogIds: number[];
  setSearchResult: (result: SearchResult) => void;
  setCurrentPage: (page: number) => void;
  setSelectedLogIds: (ids: number[]) => void;
  deleteSelectedLogs: () => void;
  deleteAllLogs: () => void;
};

export const useLogIndexStore = create<LogIndexState>((set, get) => ({
  currentPage: 1,
  searchResult: DEFAULT_SEARCH_RESULT,
  selectedLogIds: [],
  setCurrentPage: (page: number) => set({ currentPage: page }),
  setSearchResult: (result) => set({ searchResult: result }),
  setSelectedLogIds: (ids) => set({ selectedLogIds: ids }),
  deleteSelectedLogs: async () => {
    const { setSearchResult, selectedLogIds: ids } = get();

    try {
      await invoke("delete_logs", { ids });
      set({ currentPage: 1, selectedLogIds: [], searchResult: DEFAULT_SEARCH_RESULT });
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
      set({ currentPage: 1, selectedLogIds: [], searchResult: DEFAULT_SEARCH_RESULT });
      toast.success("Logs deleted successfully.");
      const result = await invoke("fetch_logs");
      setSearchResult(result as SearchResult);
    } catch (e) {
      toast.error(`Failed to delete logs: ${e}`);
    }
  },
}));
