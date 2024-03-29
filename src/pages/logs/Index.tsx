import { Log } from "@/types";
import {
  epochToLocalTime,
  millisecondsToElapsedFormat,
  translateEnemyType,
  translateEnemyTypeId,
  translateQuestId,
} from "@/utils";
import { Box, Button, Center, Checkbox, Divider, Group, Pagination, Select, Space, Table, Text } from "@mantine/core";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import useIndex from "./useIndex";

export const IndexPage = () => {
  const { t } = useTranslation();
  const {
    searchResult,
    selectedLogIds,
    setSelectedLogIds,
    setSelectedTargets,
    confirmDeleteSelected,
    confirmDeleteAll,
    handleSetPage,
    currentPage,
    setEnemyIdFilter,
    setQuestIdFilter,
  } = useIndex();

  const rows = searchResult.logs.map((log) => {
    const primaryTarget = translateEnemyType(log.primaryTarget);

    let names = "";

    if (log.version == 0) {
      names = log.name
        .split(", ")
        .map((name) => t(`characters:${name}`, `ui:characters.${name}`))
        .join(", ");
    } else {
      names = [
        { name: log.p1Name, type: log.p1Type },
        { name: log.p2Name, type: log.p2Type },
        { name: log.p3Name, type: log.p3Type },
        { name: log.p4Name, type: log.p4Type },
      ]
        .filter((player) => player.name || player.type)
        .map((player) => {
          if (!player.name) return t(`characters:${player.type}`, `ui:characters.${player.type}`);
          return `${player.name} (${t(`characters:${player.type}`, `ui:characters.${player.type}`)})`;
        })
        .join(", ");
    }

    const resetSelectedTargets = () => {
      setSelectedTargets([]);
    };

    return (
      <LogEntry
        key={log.id}
        log={log}
        selectedLogIds={selectedLogIds}
        setSelectedLogIds={setSelectedLogIds}
        primaryTarget={primaryTarget}
        names={names}
        resetSelectedTargets={resetSelectedTargets}
      />
    );
  });

  return (
    <Box>
      <Group>
        <Box style={{ display: "flex" }}>
          <Text>{t("ui.logs.saved-count", { count: searchResult.logCount })}</Text>
        </Box>
        <Box style={{ display: "flex", flexDirection: "row-reverse", flex: 1 }}>
          {selectedLogIds.length > 0 ? (
            <Button size="xs" variant="default" onClick={confirmDeleteSelected} disabled={selectedLogIds.length === 0}>
              {t("ui.logs.delete-selected-btn", { count: selectedLogIds.length })}
            </Button>
          ) : (
            <Button size="xs" variant="default" onClick={confirmDeleteAll}>
              {t("ui.logs.delete-all-btn")}
            </Button>
          )}
        </Box>
      </Group>
      <Group>
        <SelectableEnemy targetIds={searchResult.enemyIds} setSelectedTarget={setEnemyIdFilter} />
        <SelectableQuest questIds={searchResult.questIds} setSelectedQuest={setQuestIdFilter} />
      </Group>
      {searchResult.logs.length === 0 && <BlankTable />}
      {searchResult.logs.length > 0 && (
        <Box>
          <Table striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>
                  <Checkbox
                    aria-label="Select all (on page)"
                    checked={selectedLogIds.length == searchResult.logs.length}
                    onChange={(event) =>
                      setSelectedLogIds(event.currentTarget.checked ? searchResult.logs.map((log) => log.id) : [])
                    }
                  />
                </Table.Th>
                <Table.Th>{t("ui.logs.date")}</Table.Th>
                <Table.Th>{t("ui.logs.quest-name")}</Table.Th>
                <Table.Th></Table.Th>
                <Table.Th>{t("ui.logs.primary-target")}</Table.Th>
                <Table.Th>{t("ui.logs.duration")}</Table.Th>
                <Table.Th>{t("ui.logs.quest-elapsed-time")}</Table.Th>
                <Table.Th>{t("ui.logs.name")}</Table.Th>
                <Table.Th></Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>{rows}</Table.Tbody>
          </Table>
          <Divider my="sm" />
          <Pagination total={searchResult.pageCount} value={currentPage} onChange={handleSetPage} />
        </Box>
      )}
    </Box>
  );
};

function LogEntry({
  log,
  selectedLogIds,
  setSelectedLogIds,
  primaryTarget,
  names,
  resetSelectedTargets,
}: {
  log: Log;
  selectedLogIds: number[];
  setSelectedLogIds: (ids: number[]) => void;
  primaryTarget: string;
  names: string;
  resetSelectedTargets: () => void;
}): JSX.Element {
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
        <Text size="xs">{epochToLocalTime(log.time)}</Text>
      </Table.Td>
      <Table.Td>
        <Text size="xs">{translateQuestId(log.questId)}</Text>
      </Table.Td>
      <Table.Td>{log.questId && log.questCompleted !== null && (log.questCompleted ? "✓" : "X")}</Table.Td>
      <Table.Td>
        <Text size="xs">{primaryTarget}</Text>
      </Table.Td>
      <Table.Td>
        <Text size="xs">{millisecondsToElapsedFormat(log.duration)}</Text>
      </Table.Td>
      <Table.Td>
        <Text size="xs">{log.questElapsedTime ? millisecondsToElapsedFormat(log.questElapsedTime * 1000) : ""}</Text>
      </Table.Td>
      <Table.Td>
        <Text size="xs">{names}</Text>
      </Table.Td>
      <Table.Td>
        <Button size="xs" variant="default" component={Link} to={`/logs/${log.id}`} onClick={resetSelectedTargets}>
          View
        </Button>
      </Table.Td>
    </Table.Tr>
  );
}

function BlankTable() {
  const { t } = useTranslation();

  return (
    <Box>
      <Table striped highlightOnHover>
        <Table.Thead>
          <Table.Tr>
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
}

function SelectableEnemy({
  targetIds,
  setSelectedTarget,
}: {
  targetIds: number[];
  setSelectedTarget: (value: number | null) => void;
}) {
  const { t } = useTranslation();
  const targetOptions = useMemo(
    () => targetIds.map((id) => ({ value: id.toString(), label: translateEnemyTypeId(id) })),
    [targetIds]
  );

  return (
    <Select
      data={targetOptions}
      onChange={(value) => setSelectedTarget(value ? Number(value) : null)}
      placeholder={t("ui.select-enemy")}
      searchable
      clearable
    />
  );
}

function SelectableQuest({
  questIds,
  setSelectedQuest,
}: {
  questIds: number[];
  setSelectedQuest: (value: number | null) => void;
}) {
  const { t } = useTranslation();
  const questOptions = useMemo(
    () => questIds.map((id) => ({ value: id.toString(), label: translateQuestId(id) })),
    [questIds]
  );

  return (
    <Select
      data={questOptions}
      onChange={(value) => setSelectedQuest(value ? Number(value) : null)}
      placeholder={t("ui.select-quest")}
      searchable
      clearable
    />
  );
}
