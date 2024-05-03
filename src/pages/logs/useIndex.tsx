import { useEncounterStore } from "@/stores/useEncounterStore";
import { SearchResult, useLogIndexStore } from "@/stores/useLogIndexStore";
import { LogSortType } from "@/types";

import { Text } from "@mantine/core";
import { modals } from "@mantine/modals";
import { invoke } from "@tauri-apps/api";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

export default function useIndex() {
  const { t } = useTranslation();
  const [filterByEnemyId, setEnemyIdFilter] = useState<number | null>(null);
  const [filterByQuestId, setQuestIdFilter] = useState<number | null>(null);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const [sortType, setSortType] = useState<LogSortType>("time");
  const [questCompletedFilter, setQuestCompletedFilter] = useState<boolean | null>(null);

  const {
    currentPage,
    setCurrentPage,
    searchResult,
    setSearchResult,
    selectedLogIds,
    setSelectedLogIds,
    deleteSelectedLogs,
    deleteAllLogs,
  } = useLogIndexStore((state) => ({
    currentPage: state.currentPage,
    setCurrentPage: state.setCurrentPage,
    searchResult: state.searchResult,
    setSearchResult: state.setSearchResult,
    selectedLogIds: state.selectedLogIds,
    setSelectedLogIds: state.setSelectedLogIds,
    deleteSelectedLogs: state.deleteSelectedLogs,
    deleteAllLogs: state.deleteAllLogs,
  }));

  const { setSelectedTargets } = useEncounterStore((state) => ({
    setSelectedTargets: state.setSelectedTargets,
  }));

  useEffect(() => {
    invoke("fetch_logs", {
      page: currentPage,
      filterByEnemyId,
      filterByQuestId,
      sortDirection,
      sortType,
      questCompleted: questCompletedFilter,
    }).then((result) => {
      setSearchResult(result as SearchResult);
    });
  }, [currentPage, filterByEnemyId, filterByQuestId, sortDirection, sortType, questCompletedFilter]);

  useEffect(() => {
    const encounterSavedListener = listen("encounter-saved", () => {
      invoke("fetch_logs", {
        page: currentPage,
        filterByEnemyId,
        filterByQuestId,
        sortDirection,
        sortType,
        questCompleted: questCompletedFilter,
      }).then((result) => {
        setSearchResult(result as SearchResult);
      });
    });

    return () => {
      encounterSavedListener.then((f) => f());
    };
  }, [currentPage, filterByEnemyId, filterByQuestId, sortDirection, sortType, questCompletedFilter]);

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

    invoke("fetch_logs", {
      page,
      filterByEnemyId,
      filterByQuestId,
      sortDirection,
      sortType,
      questCompleted: questCompletedFilter,
    }).then((result) => {
      setSearchResult(result as SearchResult);
    });
  };

  const handleSetEnemyIdFilter = (enemyId: number | null) => {
    setCurrentPage(1);
    setEnemyIdFilter(enemyId);
  };

  const handleSetQuestIdFilter = (questId: number | null) => {
    setCurrentPage(1);
    setQuestIdFilter(questId);
  };

  const handleSetQuestCompletedFilter = (completed: boolean | null) => {
    setCurrentPage(1);
    setQuestCompletedFilter(completed);
  };

  const toggleSort = (newSortType: LogSortType) => {
    setCurrentPage(1);

    if (sortType === newSortType) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortType(newSortType);
      setSortDirection("asc");
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
    filterByEnemyId,
    filterByQuestId,
    setEnemyIdFilter: handleSetEnemyIdFilter,
    setQuestIdFilter: handleSetQuestIdFilter,
    setQuestCompletedFilter: handleSetQuestCompletedFilter,
    questCompletedFilter,
    toggleSort,
    sortType,
    sortDirection,
  };
}
