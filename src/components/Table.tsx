import { useShallow } from "zustand/react/shallow";
import { useMeterSettingsStore } from "../Store";
import { ComputedPlayerState, EncounterState, PlayerData, SortDirection, SortType } from "../types";
import { formatInPartyOrder, sortPlayers } from "../utils";
import { PlayerRow } from "./PlayerRow";

export const Table = ({
  encounterState,
  partyData,
  sortType,
  sortDirection,
  setSortType,
  setSortDirection,
}: {
  encounterState: EncounterState;
  partyData: Array<PlayerData | null>;
  sortType: SortType;
  sortDirection: SortDirection;
  setSortType: (sortType: SortType) => void;
  setSortDirection: (sortDirection: SortDirection) => void;
}) => {
  const { streamerMode } = useMeterSettingsStore(
    useShallow((state) => ({
      streamerMode: state.streamer_mode,
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
          <PlayerRow key={player.index} player={player} partyData={partyData} />
        ))}
      </tbody>
    </table>
  );
};
