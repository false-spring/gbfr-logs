import { useState } from "react";

import { CharacterType, ComputedSbaGroup, SkillMeterColumns } from "@/types";
import { getSbaCauseName } from "@/utils/i18n";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { SbaRow } from "./SbaRow";
import { getSbaColumnValue } from "./skillColumns";

export type SbaGroupRowProps = {
  characterType: CharacterType;
  group: ComputedSbaGroup;
  color: string;
  columns: SkillMeterColumns[];
};

export const SbaGroupRow = ({ characterType, group, color, columns }: SbaGroupRowProps) => {
  const [expanded, setExpanded] = useState(false);

  const sortedSources = [...group.sources].sort((a, b) => b.totalSbaAdded - a.totalSbaAdded);

  return (
    <>
      <tr className="skill-row group" onClick={() => setExpanded(!expanded)}>
        <td className="text-left row-data">
          <span>{getSbaCauseName(group.childCharacterType ?? characterType, group.cause)}</span>
          <span className="p4">{expanded ? <CaretUp size={12} /> : <CaretDown size={12} />}</span>
        </td>
        {columns.map((column) => {
          const columnValue = getSbaColumnValue(column, group);

          return (
            <td key={column} className="text-center row-data">
              {columnValue.value}
              <span className="unit font-sm">{columnValue.unit}</span>
            </td>
          );
        })}
        <div className="damage-bar" style={{ backgroundColor: color, width: `${group.percentage}%` }} />
      </tr>
      {expanded &&
        sortedSources.map((source, index) => (
          <SbaRow
            key={`${JSON.stringify(source.cause)}-${index}`}
            characterType={characterType}
            source={source}
            color={color}
            columns={columns}
            nested
          />
        ))}
    </>
  );
};
