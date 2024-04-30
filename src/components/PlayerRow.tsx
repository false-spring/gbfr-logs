import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { Fragment } from "react";

import { ComputedPlayerState, PlayerData } from "@/types";
import { translatedPlayerName } from "@/utils";

import { SkillBreakdown } from "./SkillBreakdown";
import { usePlayerRow } from "./usePlayerRow";

export const PlayerRow = ({
  live = false,
  player,
  partyData,
}: {
  live?: boolean;
  player: ComputedPlayerState;
  partyData: Array<PlayerData | null>;
}) => {
  const {
    color,
    columns,
    isOpen,
    setIsOpen,
    partySlotIndex,
    showDisplayNames,
    showFullValues,
    matchColumnTypeToValue,
  } = usePlayerRow(live, player, partyData);

  return (
    <Fragment>
      <tr className={`player-row ${isOpen ? "transparent-bg" : ""}`} onClick={() => setIsOpen(!isOpen)}>
        <td className="text-left row-data">
          {translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player, showDisplayNames)}
        </td>
        {columns.map((column) => {
          const columnValue = matchColumnTypeToValue(showFullValues, column);

          return (
            <td key={column} className="text-center row-data">
              {showFullValues ? (
                columnValue.value
              ) : (
                <>
                  {columnValue.value}
                  <span className="unit font-sm">{columnValue.unit}</span>
                </>
              )}
            </td>
          );
        })}
        <td className="text-center row-button">{isOpen ? <CaretUp size={16} /> : <CaretDown size={16} />}</td>
        <div className="damage-bar" style={{ backgroundColor: color, width: `${player.percentage}%` }} />
      </tr>
      {isOpen && <SkillBreakdown player={player} color={color} />}
    </Fragment>
  );
};
