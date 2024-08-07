import { invoke } from "@tauri-apps/api";
import toast from "react-hot-toast";
import { create } from "zustand";

import { Log, LogSortType, SortDirection } from "@/types";

export type SearchResult = {
  logs: Log[];
  page: number;
  pageCount: number;
  logCount: number;
  enemyIds: number[];
  questIds: number[];
  playerIds: string[];
  playerTypes: string[];
};

const DEFAULT_SEARCH_RESULT = {
  logs: [],
  page: 1,
  pageCount: 0,
  logCount: 0,
  enemyIds: [],
  questIds: [],
  playerIds: [],
  playerTypes: [],
};

type LogIndexState = {
  currentPage: number;
  searchResult: SearchResult;
  filters: FilterState;
  selectedLogIds: number[];
  setSearchResult: (result: SearchResult) => void;
  setFilters: (filters: Partial<FilterState>) => void;
  setCurrentPage: (page: number) => void;
  setSelectedLogIds: (ids: number[]) => void;
  deleteSelectedLogs: () => void;
  deleteAllLogs: () => void;
  fetchLogs: () => void;
};

export type FilterState = {
  filterByEnemyId: number | null;
  filterByQuestId: number | null;
  sortDirection: SortDirection;
  sortType: LogSortType;
  questCompletedFilter: boolean | null;
  filterByPlayerId: string | null;
  filterByPlayerCharacter: string | null;
  showAdvancedFilters: boolean;
};

const DEFAULT_FILTERS: FilterState = {
  filterByEnemyId: null,
  filterByQuestId: null,
  sortDirection: "desc",
  sortType: "time",
  questCompletedFilter: null,
  filterByPlayerId: null,
  filterByPlayerCharacter: null,
  showAdvancedFilters: false,
};

export const useLogIndexStore = create<LogIndexState>((set, get) => ({
  currentPage: 1,
  searchResult: DEFAULT_SEARCH_RESULT,
  filters: DEFAULT_FILTERS,
  selectedLogIds: [],
  setCurrentPage: (page: number) => set({ currentPage: page }),
  setSearchResult: (result) => set({ searchResult: result }),
  setFilters: (filters: Partial<FilterState>) =>
    set((state) => ({ currentPage: 1, filters: { ...state.filters, ...filters } })),
  setSelectedLogIds: (ids) => set({ selectedLogIds: ids }),
  deleteSelectedLogs: async () => {
    const { currentPage, searchResult, fetchLogs, selectedLogIds: ids } = get();

    try {
      await invoke("delete_logs", { ids });
      toast.success("Logs deleted successfully.");
      const newPage = Math.min(currentPage, searchResult.pageCount - 1);
      set({ currentPage: newPage, selectedLogIds: [] });
      await fetchLogs();
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
  fetchLogs: async () => {
    const { currentPage, filters, setSearchResult } = get();

    try {
      const result = await invoke("fetch_logs", {
        page: currentPage,
        filterByEnemyId: filters.filterByEnemyId,
        filterByQuestId: filters.filterByQuestId,
        filterByPlayerId: filters.filterByPlayerId,
        filterByPlayerCharacter: filters.filterByPlayerCharacter,
        sortDirection: filters.sortDirection,
        sortType: filters.sortType,
        questCompleted: filters.questCompletedFilter,
      });

      setSearchResult(result as SearchResult);
    } catch (e) {
      toast.error(`Failed to fetch logs: ${e}`);
    }
  },
}));
