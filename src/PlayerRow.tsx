import { useTranslation } from "react-i18next";
import { ComputedPlayerData } from "./types";
import { humanizeNumbers } from "./utils";

export const PlayerRow = ({
  player,
  color,
}: {
  player: ComputedPlayerData;
  color: string;
}) => {
  const { t } = useTranslation();

  const [totalDamage, totalDamageUnit] = humanizeNumbers(player.totalDamage);
  const [dps, dpsUnit] = humanizeNumbers(player.dps);

  return (
    <tr className="player-row">
      <td className="text-left row-data">
        {player.index} - {t(`characters.${player.characterType}`)}
      </td>
      <td className="text-center row-data">
        {totalDamage}
        <span className="unit font-sm">{totalDamageUnit}</span>
      </td>
      <td className="text-center row-data">
        {dps}
        <span className="unit font-sm">{dpsUnit}</span>
      </td>
      <td className="text-center row-data">
        {player.percentage.toFixed(2)}
        <span className="unit font-sm">%</span>
      </td>
      <div
        className="damage-bar"
        style={{ backgroundColor: color, width: `${player.percentage}%` }}
      />
    </tr>
  );
};
