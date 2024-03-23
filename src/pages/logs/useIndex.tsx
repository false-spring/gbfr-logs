import { useEncounterStore } from "@/stores/useEncounterStore";
import { SearchResult, useLogIndexStore } from "@/stores/useLogIndexStore";

import { Text } from "@mantine/core";
import { modals } from "@mantine/modals";
import { invoke } from "@tauri-apps/api";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

export default function useIndex() {
  const { t } = useTranslation();

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
    invoke("fetch_logs", { page: currentPage }).then((result) => {
      setSearchResult(result as SearchResult);
    });
  }, [currentPage]);

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
    invoke("fetch_logs", { page }).then((result) => {
      setSearchResult(result as SearchResult);
    });
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
  };
}
