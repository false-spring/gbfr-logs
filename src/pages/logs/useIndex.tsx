import { useEncounterStore } from "@/stores/useEncounterStore";
import { useLogIndexStore } from "@/stores/useLogIndexStore";
import { LogSortType } from "@/types";

import { Text } from "@mantine/core";
import { modals } from "@mantine/modals";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

export default function useIndex() {
  const { t } = useTranslation();

  const {
    currentPage,
    setCurrentPage,
    searchResult,
    filters,
    setFilters,
    selectedLogIds,
    setSelectedLogIds,
    deleteSelectedLogs,
    deleteAllLogs,
    fetchLogs,
  } = useLogIndexStore((state) => ({
    currentPage: state.currentPage,
    setCurrentPage: state.setCurrentPage,
    searchResult: state.searchResult,
    filters: state.filters,
    setFilters: state.setFilters,
    selectedLogIds: state.selectedLogIds,
    setSelectedLogIds: state.setSelectedLogIds,
    deleteSelectedLogs: state.deleteSelectedLogs,
    deleteAllLogs: state.deleteAllLogs,
    fetchLogs: state.fetchLogs,
  }));

  const { setSelectedTargets } = useEncounterStore((state) => ({
    setSelectedTargets: state.setSelectedTargets,
  }));

  useEffect(() => {
    fetchLogs();
  }, [currentPage, filters]);

  useEffect(() => {
    const encounterSavedListener = listen("encounter-saved", () => {
      fetchLogs();
    });

    return () => {
      encounterSavedListener.then((f) => f());
    };
  }, [currentPage, filters]);

  const confirmDeleteSelected = () =>
    modals.openConfirmModal({
      title: "Delete logs",
      children: (
        <Text size="sm">{t("ui.logs.delete-selected-logs-confirmation", { count: selectedLogIds.length })}</Text>
      ),
      labels: { confirm: t("ui.delete-btn"), cancel: t("ui.cancel-btn") },
      confirmProps: { color: "red" },
      onConfirm: () => deleteSelectedLogs(),
    });

  const confirmDeleteAll = () =>
    modals.openConfirmModal({
      title: "Delete logs",
      children: <Text size="sm">{t("ui.logs.delete-all-logs-confirmation")}</Text>,
      labels: { confirm: t("ui.delete-btn"), cancel: t("ui.cancel-btn") },
      confirmProps: { color: "red" },
      onConfirm: () => deleteAllLogs(),
    });

  const handleSetPage = (page: number) => {
    setCurrentPage(page);
    setSelectedLogIds([]);
    fetchLogs();
  };

  const toggleAdvancedFilters = () => {
    setFilters({ showAdvancedFilters: !filters.showAdvancedFilters });
  };

  const toggleSort = (newSortType: LogSortType) => {
    setCurrentPage(1);

    if (filters.sortType === newSortType) {
      setFilters({ sortDirection: filters.sortDirection === "asc" ? "desc" : "asc" });
    } else {
      setFilters({ sortType: newSortType, sortDirection: "asc" });
    }
  };

  return {
    searchResult,
    selectedLogIds,
    setSelectedLogIds,
    setSelectedTargets,
    confirmDeleteSelected,
    confirmDeleteAll,
    handleSetPage,
    currentPage,
    filters,
    setFilters,
    toggleAdvancedFilters,
    toggleSort,
  };
}
