import { CharacterType, ComputedSkillGroup, SkillMeterColumns } from "@/types";
import { skillRowKey } from "@/utils/derive";
import { getSkillName } from "@/utils/i18n";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { SkillRow } from "./SkillRow";
import { useSkillGroupRow } from "./useSkillGroupRow";

export type SkillGroupRowProps = {
  characterType: CharacterType;
  group: ComputedSkillGroup;
  color: string;
  columns: SkillMeterColumns[];
  stunPerSecondRatio: number;
};

export const SkillGroupRow = ({ characterType, group, color, columns, stunPerSecondRatio }: SkillGroupRowProps) => {
  const { getColumnValue, expanded, setExpanded, sortedSkills } = useSkillGroupRow(group, columns, stunPerSecondRatio);

  return (
    <>
      <tr className="skill-row group" onClick={() => setExpanded(!expanded)}>
        <td className="text-left row-data">
          <span>{getSkillName(group.childCharacterType, group)}</span>
          <span className="p4">{expanded ? <CaretUp size={12} /> : <CaretDown size={12} />}</span>
        </td>
        {columns.map((column) => {
          const columnValue = getColumnValue(column);

          return (
            <td key={column} className="text-center row-data">
              {columnValue.value}
              {columnValue.unit !== undefined && <span className="unit font-sm">{columnValue.unit}</span>}
            </td>
          );
        })}
        <div className="damage-bar" style={{ backgroundColor: color, width: `${group.percentage}%` }} />
      </tr>
      {expanded &&
        sortedSkills.map((skill) => (
          <SkillRow
            key={skillRowKey(skill)}
            characterType={characterType}
            skill={skill}
            color={color}
            columns={columns}
            stunPerSecondRatio={stunPerSecondRatio}
            nested
          />
        ))}
    </>
  );
};
