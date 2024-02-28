import { ComputedPlayerData, EncounterState } from "./types";
import { PlayerRow } from "./PlayerRow";

export const Table = ({
  encounterState,
}: {
  encounterState: EncounterState;
}) => {
  const colors = [
    "#FF5630",
    "#FFAB00",
    "#36B37E",
    "#00B8D9",
    "#9BCF53",
    "#380E7F",
    "#416D19",
    "#2C568D",
  ];

  let players: Array<ComputedPlayerData> = Object.keys(
    encounterState.party
  ).map((key) => {
    let playerData = encounterState.party[key];

    return {
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
      ...playerData,
    };
  });

  players.sort((a, b) => b.totalDamage - a.totalDamage);

  return (
    <table className="table w-full">
      <thead className="header transparent-bg">
        <tr>
          <th className="header-name">Name</th>
          <th className="header-column text-center">DMG</th>
          <th className="header-column text-center">DPS</th>
          <th className="header-column text-center">%</th>
        </tr>
      </thead>
      <tbody>
        {players.map((player, index) => (
          <PlayerRow key={index} player={player} color={colors[index]} />
        ))}
      </tbody>
    </table>
  );
};
