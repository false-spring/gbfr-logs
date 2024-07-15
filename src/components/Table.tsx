import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { useMeterSettingsStore } from "../stores/useMeterSettingsStore";
import { ComputedPlayerState, EncounterState, MeterColumns, PlayerData, SortDirection, SortType } from "../types";
import { formatInPartyOrder, sortPlayers } from "../utils";
import { PlayerRow } from "./PlayerRow";

export const Table = ({
  live = false,
  encounterState,
  partyData,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
}: {
  live?: boolean;
  encounterState: EncounterState;
  partyData: Array<PlayerData | null>;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
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
  let players: Array<ComputedPlayerState> = partyOrderPlayers.map((playerData) => {
    return {
      ...playerData,
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
    };
  });

  // Sort players by the selected sort type and direction
  sortPlayers(players, sortType, sortDirection);

  players = players.filter((player) => {
    const partySlotIndex = partyData.findIndex((partyMember) => partyMember?.actorIndex === player.index);

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

  // If the meter is in live mode, only show the overlay columns that are enabled, otherwise show all columns.
  const columns = live
    ? overlay_columns
    : [
        MeterColumns.TotalDamage,
        MeterColumns.DPS,
        MeterColumns.TotalStunValue,
        MeterColumns.StunPerSecond,
        MeterColumns.DamagePercentage,
      ];

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
          <PlayerRow live={live} key={player.index} player={player} partyData={partyData} />
        ))}
      </tbody>
    </table>
  );
};
