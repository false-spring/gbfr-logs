import {
  ActionIcon,
  Box,
  Button,
  Divider,
  Flex,
  Group,
  Menu,
  NumberFormatter,
  Select,
  Tabs,
  Text,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { ClipboardText } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { t } from "i18next";
import { useCallback, useEffect, useState } from "react";
import toast from "react-hot-toast";
import { Link, useParams } from "react-router-dom";

import StatusSourceNames, { StatusSourceGuesses, StatusSourceOverrides } from "@/assets/status-source-names";
import { EffectiveTraitsTab } from "@/components/EffectiveTraitsTab";
import { EquipmentTab } from "@/components/EquipmentTab";
import { MasterTraitsModal } from "@/components/MasterTraitsModal";
import { MiscTab } from "@/components/MiscTab";
import { OverviewTab } from "@/components/OverviewTab";
import { ReportIssueModal } from "@/components/ReportIssueModal";
import { SbaTab } from "@/components/SbaTab";
import { StunTab } from "@/components/StunTab";
import { UploadLogButton } from "@/components/UploadLogButton";
import { makeResolvePlayerName } from "@/components/chartCommon";
import { styleBoardSourceName } from "@/models/skillboard";
import { EncounterStateResponse, useEncounterStore } from "@/stores/useEncounterStore";
import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import {
  MeterColumns,
  type EncounterState,
  type EnemyType,
  type PlayerData,
  type SortDirection,
  type SortType,
} from "@/types";
import { PLAYER_COLORS } from "@/utils/constants";
import {
  PLAYER_ID_BASE,
  buildEnemyGroups,
  combineEncounterStates,
  formatInPartyOrder,
  resolvePartySlotIndex,
} from "@/utils/derive";
import {
  exportCharacterDataToClipboard,
  exportFullEncounterToClipboard,
  exportScreenshotToClipboard,
  exportSimpleEncounterToClipboard,
  openDamageCalculator,
} from "@/utils/export";
import { epochToLocalTime, millisecondsToElapsedFormat, toHash } from "@/utils/format";
import { translateQuestId } from "@/utils/i18n";
import {
  STATUS_KINDS_WITHOUT_MAGNITUDE,
  STATUS_SOURCE_ALL,
  buildStatusEntityOptions,
  buildStatusOptionGroups,
  buildStatusSourceOptions,
  masterTraitSourceFor,
  resolveSelectedOption,
  selectedStackSamples,
  selectedStatusSources,
  selectedStatusWindows,
  selectedValueSamples,
  statusSourceKey,
  type StatusPolarity,
} from "@/utils/status";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

import { resolveEffectiveEnemy, sumChartsAcrossEnemies } from "./enemySelection";

const formatPlayerDisplayName = (player: PlayerData, showName: boolean, showLevel: boolean = true): string => {
  const displayName = player.displayName;
  const characterType = t(`characters:${player.characterType}`, `ui:characters.${player.characterType}`);

  if (showLevel) {
    if (displayName === "" || !showName) {
      return `${characterType} Lvl. ${player.playerStats?.level || 1}`;
    } else {
      return `${displayName} (${characterType}) Lvl. ${player.playerStats?.level || 1}`;
    }
  }

  if (displayName === "" || !showName) {
    return `${characterType}`;
  } else {
    return `${displayName} (${characterType})`;
  }
};

export const ViewPage = () => {
  const {
    color_1,
    color_2,
    color_3,
    color_4,
    show_display_names,
    streamer_mode,
    show_link_time,
    show_sba_chain,
    show_break,
    setMeterSettings,
  } = useMeterSettingsStore(
    useShallow((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
      show_display_names: state.show_display_names,
      streamer_mode: state.streamer_mode,
      show_link_time: state.show_link_time,
      show_sba_chain: state.show_sba_chain,
      show_break: state.show_break,
      setMeterSettings: state.set,
    }))
  );
  const playerColors = [color_1, color_2, color_3, color_4, ...PLAYER_COLORS.slice(4)];

  const { t, i18n } = useTranslation();
  const { id } = useParams();

  const {
    encounterGlobal,
    encounterStatesByEnemy,
    dpsChartByEnemy,
    stunChartByEnemy,
    stunResetByEnemy,
    sbaChart,
    sbaEvents,
    healProvidedChart,
    healReceivedChart,
    damageTakenChart,
    deaths,
    miscChartLen,
    linkTimeWindows,
    confluxAreaClears,
    confluxBossClears,
    sbaWindows,
    breakWindows,
    statusIntervals,
    statusPeakStacks,
    statusStackSeries,
    statusValueSeries,
    statusValueIsFraction,
    statusSources,
    chartLen,
    sbaChartLen,
    questId,
    questTimer,
    questCompleted,
    playerData,
    loadFromResponse,
    fetchEnemyState,
  } = useEncounterStore((state) => ({
    encounterGlobal: state.encounterState,
    encounterStatesByEnemy: state.encounterStatesByEnemy,
    dpsChartByEnemy: state.dpsChartByEnemy,
    stunChartByEnemy: state.stunChartByEnemy,
    stunResetByEnemy: state.stunResetByEnemy,
    sbaChart: state.sbaChart,
    sbaEvents: state.sbaEvents,
    healProvidedChart: state.healProvidedChart,
    healReceivedChart: state.healReceivedChart,
    damageTakenChart: state.damageTakenChart,
    deaths: state.deaths,
    miscChartLen: state.miscChartLen,
    linkTimeWindows: state.linkTimeWindows,
    confluxAreaClears: state.confluxAreaClears,
    confluxBossClears: state.confluxBossClears,
    sbaWindows: state.sbaWindows,
    breakWindows: state.breakWindows,
    statusIntervals: state.statusIntervals,
    statusPeakStacks: state.statusPeakStacks,
    statusStackSeries: state.statusStackSeries,
    statusValueSeries: state.statusValueSeries,
    statusValueIsFraction: state.statusValueIsFraction,
    statusSources: state.statusSources,
    chartLen: state.chartLen,
    sbaChartLen: state.sbaChartLen,
    playerData: state.players,
    questId: state.questId,
    questTimer: state.questTimer,
    questCompleted: state.questCompleted,
    loadFromResponse: state.loadFromResponse,
    fetchEnemyState: state.fetchEnemyState,
  }));
  const [sortType, setSortType] = useState<SortType>(MeterColumns.TotalDamage);
  const [sortDirection, setSortDirection] = useState<SortDirection>("desc");
  const [stunSortType, setStunSortType] = useState<SortType>(MeterColumns.TotalStunValue);
  const [sbaSortType, setSbaSortType] = useState<SortType>(MeterColumns.TotalSbaAdded);
  const [stunSortDirection, setStunSortDirection] = useState<SortDirection>("desc");
  const [sbaSortDirection, setSbaSortDirection] = useState<SortDirection>("desc");
  const [detailsOpened, detailsHandlers] = useDisclosure(false);
  const [detailsPlayer, setDetailsPlayer] = useState<PlayerData | null>(null);
  const [selectedEnemy, setSelectedEnemy] = useState<number | "all" | null>(null);
  const [selectedStatusActor, setSelectedStatusActor] = useState<number | null>(null);
  const [selectedStatusKind, setSelectedStatusKind] = useState<number | null>(null);
  const [selectedStatusSource, setSelectedStatusSource] = useState<string>(STATUS_SOURCE_ALL);

  useEffect(() => {
    invoke("fetch_encounter_state", { id: Number(id) })
      .then((result) => {
        loadFromResponse(result as EncounterStateResponse);
      })
      .catch((e) => {
        toast.error(`Failed to fetch encounter state: ${e}`);
      });
  }, [id]);

  useEffect(() => {
    if (!encounterGlobal || id === undefined) return;
    const groups = buildEnemyGroups(Object.values(encounterGlobal.targets));
    const effective = resolveEffectiveEnemy(
      selectedEnemy,
      groups.map((group) => group.representativeIndex)
    );
    if (typeof effective !== "number") return;
    const group = groups.find((g) => g.representativeIndex === effective);
    for (const memberIndex of group?.indices ?? [effective]) fetchEnemyState(Number(id), memberIndex);
  }, [encounterGlobal, selectedEnemy, id, fetchEnemyState]);

  const setOverlayVisible = useCallback(
    (overlay: "link-time" | "sba-chain" | "break", visible: boolean) => {
      if (overlay === "link-time") setMeterSettings({ show_link_time: visible });
      else if (overlay === "sba-chain") setMeterSettings({ show_sba_chain: visible });
      else setMeterSettings({ show_break: visible });
    },
    [setMeterSettings]
  );

  const exportDisplayNames = show_display_names && !streamer_mode;

  const handleCharacterDataCopy = useCallback(
    (player: PlayerData) => {
      if (player) exportCharacterDataToClipboard(player, exportDisplayNames);
    },
    [exportDisplayNames]
  );

  const handleOpenDamageCalculator = useCallback((player: PlayerData) => {
    if (player) openDamageCalculator(player);
  }, []);

  const handleScreenshotCopy = useCallback(() => {
    exportScreenshotToClipboard("#log-view-page");
  }, []);

  if (!encounterGlobal) {
    return (
      <Box>
        <Text>
          <Link to="/logs">{t("ui.back-btn")}</Link>
        </Text>
        <Divider my="sm" />
        <Text>Loading...</Text>
      </Box>
    );
  }

  const enemies = Object.values(encounterGlobal.targets).sort((a, b) => b.totalDamage - a.totalDamage);
  const enemyGroups = buildEnemyGroups(enemies);
  const effectiveEnemy = resolveEffectiveEnemy(
    selectedEnemy,
    enemyGroups.map((group) => group.representativeIndex)
  );
  const showAllEnemies = effectiveEnemy === "all";
  const selectedGroup =
    typeof effectiveEnemy === "number"
      ? enemyGroups.find((group) => group.representativeIndex === effectiveEnemy)
      : undefined;
  const selectedIndices = selectedGroup?.indices ?? (typeof effectiveEnemy === "number" ? [effectiveEnemy] : []);

  const selectedStates = selectedIndices
    .map((index) => encounterStatesByEnemy[index])
    .filter((state): state is EncounterState => state !== undefined);

  const encounter = showAllEnemies
    ? encounterGlobal
    : selectedIndices.length > 0 && selectedStates.length === selectedIndices.length
      ? selectedStates.length === 1
        ? selectedStates[0]
        : combineEncounterStates(selectedStates)
      : encounterGlobal;
  const enemyDps = showAllEnemies
    ? sumChartsAcrossEnemies(dpsChartByEnemy)
    : sumChartsAcrossEnemies(dpsChartByEnemy, selectedIndices);
  const enemyStun = showAllEnemies
    ? sumChartsAcrossEnemies(stunChartByEnemy)
    : sumChartsAcrossEnemies(stunChartByEnemy, selectedIndices);
  const enemyStunResets =
    !showAllEnemies && selectedIndices.length === 1 ? stunResetByEnemy[selectedIndices[0]] ?? [] : [];

  const namedEnemy = (et: EnemyType): string | null => {
    if (typeof et == "object" && Object.hasOwn(et, "Unknown")) {
      const hash = et.Unknown.toString(16).padStart(8, "0");
      return i18n.exists(`enemies:${hash}.text`) ? t(`enemies:${hash}.text`) : null;
    }
    return i18n.exists(`enemies.${et}`) ? t(`enemies.${et}`) : null;
  };
  const unknownEnemyLabel = (et: EnemyType, baseEt: EnemyType): string => {
    const toHash = (e: EnemyType) => (typeof e == "object" ? e.Unknown.toString(16).padStart(8, "0") : String(e));
    const hash = toHash(et);
    const baseHash = toHash(baseEt);
    return t([`enemies.unknown.${hash}`, `enemies.unknown.${baseHash}`, "enemies.unknown-type"], { id: hash });
  };

  const enemyItems = enemyGroups.map((group) => {
    const enemy = group.representative;
    const label =
      namedEnemy(enemy.targetType) ??
      namedEnemy(enemy.baseTargetType) ??
      unknownEnemyLabel(enemy.targetType, enemy.baseTargetType);
    return { value: String(group.representativeIndex), label };
  });
  if (enemyGroups.length > 1) {
    enemyItems.unshift({ value: "all", label: t("ui.logs.all-enemies", "All") });
  }

  const players = formatInPartyOrder(encounter.party);
  // Gauge generation is not enemy-attributable, so the SBA tab always reads
  // the whole-encounter state.
  const sbaPlayers = formatInPartyOrder(encounterGlobal.party);

  const resolveStatusPlayerName = makeResolvePlayerName(sbaPlayers, playerData, show_display_names, streamer_mode);
  const statusEntityLabel = (actorId: number): string => {
    // Fail closed: a label resolver must never white-screen the page.
    if (typeof actorId !== "number" || !Number.isFinite(actorId)) return "Unknown";
    if (actorId >= PLAYER_ID_BASE) return resolveStatusPlayerName(actorId);
    const target = encounterGlobal.targets[actorId];
    if (!target) return `0x${actorId.toString(16).padStart(8, "0")}`;
    return (
      namedEnemy(target.targetType) ??
      namedEnemy(target.baseTargetType) ??
      unknownEnemyLabel(target.targetType, target.baseTargetType)
    );
  };

  const statusEntityItems = buildStatusEntityOptions(statusIntervals, statusEntityLabel);
  const statusActor = statusEntityItems.some((item) => Number(item.value) === selectedStatusActor)
    ? selectedStatusActor
    : null;
  const statusGroups = buildStatusOptionGroups(statusIntervals, statusActor, statusPeakStacks);
  const statusKind = statusGroups.some((group) => group.items.some((item) => Number(item.value) === selectedStatusKind))
    ? selectedStatusKind
    : null;

  const statusSourceActionName = (
    applierIndex: number,
    actionId: number,
    magnitude: number | undefined
  ): string | null => {
    const partySlotIndex = resolvePartySlotIndex(playerData, applierIndex);
    const characterType = playerData[partySlotIndex]?.characterType;

    if (typeof characterType === "string" && statusKind !== null) {
      const override = StatusSourceOverrides[`${characterType.toUpperCase()}:${statusKind}:${actionId}`];
      if (override) return override;
    }

    if (characterType) {
      const key = `skills.${characterType}.${actionId}`;
      const name = t(key);
      // i18next echoes the key back when there is no entry for it.
      if (name !== key) return name;
    }

    if (statusKind === null) return null;
    const pair = `${actionId}:${statusKind}`;

    const confirmed = StatusSourceNames[pair];
    if (confirmed) return confirmed;

    const guess = StatusSourceGuesses[pair];
    if (guess) return `${guess}?`;

    const perk = masterTraitSourceFor(
      typeof characterType === "string" ? characterType : undefined,
      statusKind,
      magnitude,
      statusValueIsFraction[statusKind]
    );
    if (perk) return `${perk} (Master Trait)`;

    return styleBoardSourceName(actionId, playerData[partySlotIndex]?.masterTraitFlags);
  };

  const statusSourceList = selectedStatusSources(statusSources, statusActor, statusKind);
  const statusSourceOptions = buildStatusSourceOptions(
    statusSourceList,
    statusEntityLabel,
    t("ui.logs.status-source-all", "All"),
    t("ui.logs.status-source-unknown", "Unknown"),
    statusSourceActionName,
    statusKind !== null ? statusValueIsFraction[statusKind] : undefined
  );
  const statusSource = resolveSelectedOption(statusSourceOptions, selectedStatusSource);
  const pickedSource =
    statusSource === STATUS_SOURCE_ALL
      ? null
      : statusSourceList.find((source) => statusSourceKey(source) === statusSource) ?? null;

  const statusWindows = pickedSource
    ? pickedSource.windows
    : selectedStatusWindows(statusIntervals, statusActor, statusKind);
  const statusStackSamples = selectedStackSamples(statusStackSeries, statusActor, statusKind);
  const statusValueSamples =
    statusKind !== null && STATUS_KINDS_WITHOUT_MAGNITUDE.has(statusKind)
      ? []
      : pickedSource && pickedSource.values.length > 0
        ? pickedSource.values
        : selectedValueSamples(statusValueSeries, statusActor, statusKind);

  const statusGroupLabels: Record<StatusPolarity, string> = {
    buff: t("ui.logs.status-buffs", "Buffs"),
    debuff: t("ui.logs.status-ailments", "Ailments"),
    other: t("ui.logs.status-uncatalogued", "Uncatalogued"),
  };
  const statusSelect =
    statusEntityItems.length > 0 ? (
      <Group gap="xs" w="100%">
        <Select
          data={statusEntityItems}
          value={statusActor !== null ? String(statusActor) : null}
          onChange={(value) => {
            setSelectedStatusActor(value !== null ? Number(value) : null);
            setSelectedStatusKind(null);
          }}
          clearable
          placeholder={t("ui.logs.select-status-entity", "Status on")}
          maw={260}
        />
        <Select
          data={statusGroups.map((group) => ({ group: statusGroupLabels[group.polarity], items: group.items }))}
          value={statusKind !== null ? String(statusKind) : null}
          onChange={(value) => {
            setSelectedStatusKind(value !== null ? Number(value) : null);
            setSelectedStatusSource(STATUS_SOURCE_ALL);
          }}
          clearable
          disabled={statusActor === null}
          placeholder={t("ui.logs.select-status", "Status effect")}
          maw={260}
        />
        {statusSourceList.length > 0 ? (
          <Select
            data={statusSourceOptions}
            value={statusSource}
            onChange={(value) => setSelectedStatusSource(value ?? STATUS_SOURCE_ALL)}
            label={undefined}
            placeholder={t("ui.logs.select-status-source", "Applied by")}
            style={{ flex: 1, minWidth: 0 }}
          />
        ) : null}
      </Group>
    ) : null;

  // An empty target list reads as "every target" to the parser's CSV export.
  const selectedTargetTypes = showAllEnemies
    ? []
    : selectedIndices
        .map((index) => encounterGlobal.targets[index]?.targetType)
        .filter((targetType): targetType is EnemyType => targetType !== undefined);

  const handleSimpleEncounterCopy = () =>
    exportSimpleEncounterToClipboard(sortType, sortDirection, encounter, playerData, exportDisplayNames);

  const handleFullEncounterCopy = () =>
    exportFullEncounterToClipboard(sortType, sortDirection, encounter, playerData, exportDisplayNames);

  const exportDamageLogToFile = () => {
    if (id) invoke("export_damage_log_to_file", { id: Number(id), options: { targets: selectedTargetTypes } });
  };

  const enemySelect = (
    <Select
      data={enemyItems}
      value={effectiveEnemy !== null ? String(effectiveEnemy) : null}
      onChange={(value) => setSelectedEnemy(value === "all" ? "all" : value !== null ? Number(value) : null)}
      allowDeselect={false}
      placeholder={t("ui.logs.select-enemy", "Select enemy")}
      disabled={enemyItems.length === 0}
      maw={320}
    />
  );

  const playerNames = playerData.map((player) =>
    formatPlayerDisplayName(player, show_display_names && !streamer_mode, false)
  );

  return (
    <Box>
      <Box>
        <Box display="flex">
          <Box display="flex" flex={1}>
            <Button size="xs" variant="default" component={Link} to="/logs">
              {t("ui.back-btn")}
            </Button>
          </Box>
          <Flex display="flex" flex={1} justify={"flex-end"} align="center" gap="xs">
            <UploadLogButton id={id} />
            <ReportIssueModal id={id} />
            <Menu shadow="md" trigger="hover" openDelay={100} closeDelay={400}>
              <Menu.Target>
                <ActionIcon aria-label="Clipboard" variant="filled" color="light">
                  <ClipboardText size={16} />
                </ActionIcon>
              </Menu.Target>
              <Menu.Dropdown>
                <Menu.Item onClick={handleSimpleEncounterCopy}>{t("ui.copy-to-clipboard-simple")}</Menu.Item>
                <Menu.Item onClick={handleFullEncounterCopy}>{t("ui.copy-to-clipboard-full")}</Menu.Item>
                <Menu.Item onClick={handleScreenshotCopy}>{t("ui.copy-screenshot-to-clipboard")}</Menu.Item>
                <Menu.Item onClick={exportDamageLogToFile}>{t("ui.export-damage-log")}</Menu.Item>
              </Menu.Dropdown>
            </Menu>
          </Flex>
        </Box>
      </Box>

      <Divider my="sm" />

      <Box id="log-view-page">
        <Box>
          {questId && (
            <Box display="flex">
              <Text size="sm" fw={800}>
                {t("ui.logs.quest-name")}:
              </Text>
              <Text size="sm" ml={4}>
                {translateQuestId(questId)} ({toHash(questId)}){" "}
              </Text>
            </Box>
          )}
          {questId && (
            <Box display="flex">
              <Text size="sm" fw={800}>
                {t("ui.logs.quest-status")}:
              </Text>
              <Text size="sm" fs="italic" ml={4}>
                {questCompleted ? "✅" : "❌"}
              </Text>
            </Box>
          )}
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.date")}:
            </Text>
            <Text size="sm" fs="italic" ml={4}>
              {epochToLocalTime(encounter.startTime)}
            </Text>
          </Box>
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.duration")}:
            </Text>
            <Text size="sm" fs="italic" ml={4}>
              {millisecondsToElapsedFormat(encounter.endTime - encounter.startTime)}
            </Text>
          </Box>
          {questTimer && (
            <Box display="flex">
              <Text size="sm" fw={800}>
                {t("ui.logs.quest-elapsed-time")}:
              </Text>
              <Text size="sm" fs="italic" ml={4}>
                {millisecondsToElapsedFormat(questTimer * 1000)}
              </Text>
            </Box>
          )}
          <Box display="flex">
            <Text size="sm" fw={800}>
              {t("ui.logs.total-damage")}:
            </Text>
            <Text size="sm" fs="italic" ml={4}>
              <NumberFormatter thousandSeparator value={encounter.totalDamage} />
            </Text>
          </Box>
        </Box>

        <Divider my="sm" />

        <Tabs defaultValue="overview" variant="outline">
          <Tabs.List>
            <Tabs.Tab value="overview">{t("ui.logs.overview")}</Tabs.Tab>
            <Tabs.Tab value="sba">{t("ui.logs.sba-chart")}</Tabs.Tab>
            <Tabs.Tab value="stun">{t("ui.logs.stun-chart", "Stun")}</Tabs.Tab>
            <Tabs.Tab value="equipment" disabled={playerData.length === 0}>
              {t("ui.logs.equipment")}
            </Tabs.Tab>
            <Tabs.Tab
              value="effective-traits"
              disabled={!playerData.some((player) => (player.effectiveTraits?.length ?? 0) > 0)}
            >
              {t("ui.logs.effective-traits", "Effective Traits")}
            </Tabs.Tab>
            <Tabs.Tab value="misc">{t("ui.logs.misc", "Misc")}</Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel value="overview">
            <OverviewTab
              encounter={encounter}
              sortType={sortType}
              sortDirection={sortDirection}
              setSortType={setSortType}
              setSortDirection={setSortDirection}
              playerData={playerData}
              players={players}
              showDisplayNames={show_display_names}
              streamerMode={streamer_mode}
              playerColors={playerColors}
              chartLen={chartLen}
              enemyDps={enemyDps}
              linkTimeWindows={linkTimeWindows}
              confluxAreaClears={confluxAreaClears}
              confluxBossClears={confluxBossClears}
              sbaWindows={sbaWindows}
              breakWindows={breakWindows}
              showLinkTime={show_link_time}
              showSbaChain={show_sba_chain}
              showBreak={show_break}
              setOverlayVisible={setOverlayVisible}
              enemySelect={enemySelect}
              statusSelect={statusSelect}
              statusStackSamples={statusStackSamples}
              statusValueSamples={statusValueSamples}
              statusValueIsFraction={statusKind !== null ? statusValueIsFraction[statusKind] : undefined}
              statusWindows={statusWindows}
            />
          </Tabs.Panel>
          <Tabs.Panel value="sba">
            <SbaTab
              playerData={playerData}
              players={sbaPlayers}
              encounter={encounterGlobal}
              sortType={sbaSortType}
              sortDirection={sbaSortDirection}
              setSortType={setSbaSortType}
              setSortDirection={setSbaSortDirection}
              showDisplayNames={show_display_names}
              streamerMode={streamer_mode}
              playerColors={playerColors}
              sbaChartLen={sbaChartLen}
              sbaChart={sbaChart}
              sbaEvents={sbaEvents}
              linkTimeWindows={linkTimeWindows}
              confluxAreaClears={confluxAreaClears}
              confluxBossClears={confluxBossClears}
              sbaWindows={sbaWindows}
              breakWindows={breakWindows}
              showLinkTime={show_link_time}
              showSbaChain={show_sba_chain}
              showBreak={show_break}
            />
          </Tabs.Panel>
          <Tabs.Panel value="stun">
            <StunTab
              encounter={encounter}
              sortType={stunSortType}
              sortDirection={stunSortDirection}
              setSortType={setStunSortType}
              setSortDirection={setStunSortDirection}
              playerData={playerData}
              players={players}
              showDisplayNames={show_display_names}
              streamerMode={streamer_mode}
              playerColors={playerColors}
              chartLen={chartLen}
              enemyStun={enemyStun}
              enemyStunResets={enemyStunResets}
              confluxAreaClears={confluxAreaClears}
              confluxBossClears={confluxBossClears}
              sbaWindows={sbaWindows}
              breakWindows={breakWindows}
              showSbaChain={show_sba_chain}
              showBreak={show_break}
              enemySelect={enemySelect}
            />
          </Tabs.Panel>
          <Tabs.Panel value="equipment">
            <EquipmentTab
              playerData={playerData}
              playerNames={playerNames}
              onCopyCharacterData={handleCharacterDataCopy}
              onOpenDamageCalculator={handleOpenDamageCalculator}
              onShowMasterTraitsDetails={(player) => {
                setDetailsPlayer(player);
                detailsHandlers.open();
              }}
            />
          </Tabs.Panel>
          <Tabs.Panel value="effective-traits">
            <EffectiveTraitsTab playerData={playerData} playerNames={playerNames} />
          </Tabs.Panel>
          <Tabs.Panel value="misc">
            <MiscTab
              playerData={playerData}
              // Global party, not the enemy-filtered `players`: the per-enemy
              // derived state carries no healing.
              players={sbaPlayers}
              showDisplayNames={show_display_names}
              streamerMode={streamer_mode}
              playerColors={playerColors}
              miscChartLen={miscChartLen}
              healProvidedChart={healProvidedChart}
              healReceivedChart={healReceivedChart}
              damageTakenChart={damageTakenChart}
              deaths={deaths}
            />
          </Tabs.Panel>
        </Tabs>
      </Box>
      <MasterTraitsModal opened={detailsOpened} onClose={detailsHandlers.close} player={detailsPlayer} />
    </Box>
  );
};
