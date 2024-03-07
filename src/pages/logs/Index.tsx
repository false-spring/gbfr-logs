import { Box, Button, Divider, Group, Table, Text, Pagination, Space, Center, Checkbox } from "@mantine/core";
import { invoke } from "@tauri-apps/api";
import { useEffect } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { epochToLocalTime, millisecondsToElapsedFormat } from "../../utils";
import { useTranslation } from "react-i18next";
import { useLogIndexStore, useEncounterStore, SearchResult } from "../Logs";
import { modals } from "@mantine/modals";

export const IndexPage = () => {
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
            setSearchResult(result)
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

  const rows = searchResult.logs.map((log) => {
    const names = log.name
      .split(", ")
      .map((name) => t(`characters.${name}`))
      .join(", ");

    const resetSelectedTargets = () => {
      setSelectedTargets([]);
    };

    return (
      <Table.Tr key={log.id}>
        <Table.Td>
          <Checkbox
            aria-label="Select row"
            checked={selectedLogIds.includes(log.id)}
            onChange={(event) =>
              setSelectedLogIds(
                event.currentTarget.checked ? [...selectedLogIds, log.id] : selectedLogIds.filter((id) => id !== log.id)
              )
            }
          />
        </Table.Td>
        <Table.Td>
          <Text size="sm">{epochToLocalTime(log.time)}</Text>
        </Table.Td>
        <Table.Td>
          {millisecondsToElapsedFormat(log.duration)} - {names}
        </Table.Td>
        <Table.Td>
          <Button size="xs" variant="default" component={Link} to={`/logs/${log.id}`} onClick={resetSelectedTargets}>
            View
          </Button>
        </Table.Td>
      </Table.Tr>
    );
  });

  if (searchResult.logs.length === 0) {
    return (
      <Box>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>{t("ui.logs.date")}</Table.Th>
              <Table.Th>{t("ui.logs.name")}</Table.Th>
              <Table.Th></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody></Table.Tbody>
        </Table>
        <Space h="sm" />
        <Center>
          <Text>{t("ui.logs.saved-count", { count: 0 })}</Text>
        </Center>
        <Divider my="sm" />
        <Pagination total={1} disabled />
      </Box>
    );
  } else {
    return (
      <Box>
        <Group>
          <Box style={{ display: "flex" }}>
            <Text>{t("ui.logs.saved-count", { count: searchResult.logCount })}</Text>
          </Box>
          <Box style={{ display: "flex", flexDirection: "row-reverse", flex: 1 }}>
            {selectedLogIds.length > 0 ? (
              <Button
                size="xs"
                variant="default"
                onClick={confirmDeleteSelected}
                disabled={selectedLogIds.length === 0}
              >
                {t("ui.logs.delete-selected-btn")}
              </Button>
            ) : (
              <Button size="xs" variant="default" onClick={confirmDeleteAll}>
                {t("ui.logs.delete-all-btn")}
              </Button>
            )}
          </Box>
        </Group>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th />
              <Table.Th>{t("ui.logs.date")}</Table.Th>
              <Table.Th>{t("ui.logs.name")}</Table.Th>
              <Table.Th></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>{rows}</Table.Tbody>
        </Table>
        <Divider my="sm" />
        <Pagination total={searchResult.pageCount} value={currentPage} onChange={handleSetPage} />
      </Box>
    );
  }
};
