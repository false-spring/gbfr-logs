import { ComputedPlayerData, EncounterState } from "../types";
import { PLAYER_COLORS, formatInPartyOrder } from "../utils";
import { PlayerRow } from "./PlayerRow";

export const Table = ({ encounterState }: { encounterState: EncounterState }) => {
  const partyOrderPlayers = formatInPartyOrder(encounterState.party);
  const players: Array<ComputedPlayerData> = partyOrderPlayers.map((playerData) => {
    return {
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
      ...playerData,
    };
  });

  players.sort((a, b) => b.totalDamage - a.totalDamage);

  return (
    <table className="player-table table w-full">
      <thead className="header transparent-bg">
        <tr>
          <th className="header-name">Name</th>
          <th className="header-column text-center">DMG</th>
          <th className="header-column text-center">DPS</th>
          <th className="header-column text-center">%</th>
          <th className="header-column text-center dropdown" style={{ width: "2em" }}></th>
        </tr>
      </thead>
      <tbody>
        {players.map((player) => (
          <PlayerRow key={player.index} player={player} color={PLAYER_COLORS[player.partyIndex]} />
        ))}
      </tbody>
    </table>
  );
};
