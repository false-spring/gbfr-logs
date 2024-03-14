import { useShallow } from "zustand/react/shallow";
import { useMeterSettingsStore } from "../Store";
import { ComputedPlayerState, EncounterState, SortDirection, SortType } from "../types";
import { formatInPartyOrder, sortPlayers } from "../utils";
import { PlayerRow } from "./PlayerRow";

export const Table = ({
  encounterState,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
}: {
  encounterState: EncounterState;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
}) => {
  const { color_1, color_2, color_3, color_4 } = useMeterSettingsStore(
    useShallow((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
    }))
  );

  const player_colors = [color_1, color_2, color_3, color_4];

  const partyOrderPlayers = formatInPartyOrder(encounterState.party);
  const players: Array<ComputedPlayerState> = partyOrderPlayers.map((playerData) => {
    return {
      ...playerData,
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
    };
  });

  // Sort players by the selected sort type and direction
  sortPlayers(players, sortType, sortDirection);

  const toggleSort = (newSortType: SortType) => {
    if (sortType === newSortType) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortType(newSortType);
      setSortDirection("asc");
    }
  };

  return (
    <table className="player-table table w-full">
      <thead className="header transparent-bg">
        <tr>
          <th className="header-name" onClick={() => toggleSort("partyIndex")}>
            Name
          </th>
          <th className="header-column text-center" onClick={() => toggleSort("damage")}>
            DMG
          </th>
          <th className="header-column text-center" onClick={() => toggleSort("dps")}>
            DPS
          </th>
          <th className="header-column text-center" onClick={() => toggleSort("percentage")}>
            %
          </th>
          <th className="header-column text-center dropdown" style={{ width: "2em" }}></th>
        </tr>
      </thead>
      <tbody>
        {players.map((player) => (
          <PlayerRow key={player.index} player={player} color={player_colors[player.partyIndex]} />
        ))}
      </tbody>
    </table>
  );
};
