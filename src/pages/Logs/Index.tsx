import {
    AppShell,
    Box,
    Burger,
    Button,
    Divider,
    Group,
    NavLink,
    Table,
    Text,
    Pagination,
    Fieldset,
    Select,
    Stack,
    Space,
    Center,
    NumberFormatter,
    Paper,
    Checkbox,
    MultiSelect,
    Menu,
    ActionIcon,
    Flex,
} from "@mantine/core";

import {
    PLAYER_COLORS,
    epochToLocalTime,
    exportFullEncounterToClipboard,
    exportSimpleEncounterToClipboard,
    formatInPartyOrder,
    humanizeNumbers,
    millisecondsToElapsedFormat,
    translatedPlayerName,
} from "../../utils";

import { useCallback, useEffect } from "react";
import { Link, Outlet, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useLogIndexStore, useEncounterStore, SearchResult } from "../Logs";
import { invoke } from "@tauri-apps/api";
import { listen } from "@tauri-apps/api/event";

const IndexPage = () => {
    const { t } = useTranslation();
    const {
      currentPage,
      setCurrentPage,
      searchResult,
      setSearchResult,
      selectedLogIds,
      setSelectedLogIds,
      deleteSelectedLogs,
    } = useLogIndexStore((state) => ({
      currentPage: state.currentPage,
      setCurrentPage: state.setCurrentPage,
      searchResult: state.searchResult,
      setSearchResult: state.setSearchResult,
      selectedLogIds: state.selectedLogIds,
      setSelectedLogIds: state.setSelectedLogIds,
      deleteSelectedLogs: state.deleteSelectedLogs,
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
              <Button size="xs" variant="default" onClick={deleteSelectedLogs} disabled={selectedLogIds.length === 0}>
                {t("ui.logs.delete-selected-btn")}
              </Button>
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

  export { IndexPage };