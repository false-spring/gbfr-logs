import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { useMeterSettingsStore } from "../stores/useMeterSettingsStore";
import { ComputedPlayerState, LiveEncounterState, MeterColumns, PlayerData, SortDirection, SortType } from "../types";
import { formatInPartyOrder, resolvePartySlotIndex, sortPlayers } from "../utils/derive";
import { PlayerRow } from "./PlayerRow";
import { METER_DAMAGE_COLUMNS, METER_SBA_COLUMNS, METER_STUN_COLUMNS } from "./skillColumns";

export const Table = ({
  live = false,
  encounterState,
  partyData,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
  metric = "damage",
}: {
  live?: boolean;
  encounterState: LiveEncounterState;
  partyData: Array<PlayerData | null>;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
  metric?: "damage" | "stun" | "sba";
}) => {
  const { t } = useTranslation();
  const { streamerMode, show_full_values, overlay_columns } = useMeterSettingsStore(
    useShallow((state) => ({
      useCondensedSkills: state.use_condensed_skills,
      streamerMode: state.streamer_mode,
      show_full_values: state.show_full_values,
      overlay_columns: state.overlay_columns,
    }))
  );

  const partyOrderPlayers = formatInPartyOrder(encounterState.party);
  const totalStunValue = Object.values(encounterState.party).reduce((acc, p) => acc + p.totalStunValue, 0);
  const totalSbaAdded = Object.values(encounterState.party).reduce((acc, p) => acc + (p.totalSbaAdded ?? 0), 0);
  let players: Array<ComputedPlayerState> = partyOrderPlayers.map((playerData) => {
    return {
      ...playerData,
      percentage:
        metric === "sba"
          ? totalSbaAdded > 0
            ? ((playerData.totalSbaAdded ?? 0) / totalSbaAdded) * 100
            : 0
          : metric === "stun"
            ? totalStunValue > 0
              ? (playerData.totalStunValue / totalStunValue) * 100
              : 0
            : (playerData.totalDamage / encounterState.totalDamage) * 100,
    };
  });

  // Sort players by the selected sort type and direction
  sortPlayers(players, sortType, sortDirection);

  players = players.filter((player) => {
    const partySlotIndex = resolvePartySlotIndex(partyData, player.index);

    // If streamer mode is ON, then only show the first party slot (the streamer's character)
    // Otherwise, show all players.
    return streamerMode ? partySlotIndex === 0 : true;
  });

  const toggleSort = (newSortType: SortType) => {
    if (sortType === newSortType) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortType(newSortType);
      setSortDirection("asc");
    }
  };

  const columns = live
    ? overlay_columns
    : metric === "sba"
      ? METER_SBA_COLUMNS
      : metric === "stun"
        ? METER_STUN_COLUMNS
        : METER_DAMAGE_COLUMNS;

  return (
    <table className={`player-table table w-full ${show_full_values ? "full-values" : ""}`}>
      <thead className="header transparent-bg">
        <tr>
          <th className="header-name" onClick={() => toggleSort(MeterColumns.Name)}>
            Name
          </th>
          {columns.map((column) => (
            <th key={column} className="header-column text-center" onClick={() => toggleSort(column)}>
              {t(`ui.meter-columns.${column}`)}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {players.map((player) => (
          <PlayerRow
            live={live}
            key={player.index}
            player={player}
            partyData={partyData}
            partyTotalStunValue={totalStunValue}
            metric={metric}
          />
        ))}
      </tbody>
    </table>
  );
};
