import { CharacterType, ComputedSkillState, SkillMeterColumns } from "@/types";
import { getSkillName } from "@/utils/i18n";
import { useSkillRow } from "./useSkillRow";

export type SkillRowProps = {
  characterType: CharacterType;
  skill: ComputedSkillState;
  color: string;
  columns: SkillMeterColumns[];
  stunPerSecondRatio: number;
  nested?: boolean;
};

export const SkillRow = ({ characterType, skill, color, columns, stunPerSecondRatio, nested }: SkillRowProps) => {
  const { getColumnValue } = useSkillRow(skill, stunPerSecondRatio);

  return (
    <tr className={`skill-row ${nested ? "nested" : ""}`}>
      {nested ? (
        <td className="text-left row-data nested">{getSkillName(characterType, skill)}</td>
      ) : (
        <td className="text-left row-data">{getSkillName(characterType, skill)}</td>
      )}
      {columns.map((column) => {
        const columnValue = getColumnValue(column);

        return (
          <td key={column} className="text-center row-data">
            {columnValue.value}
            {columnValue.unit !== undefined && <span className="unit font-sm">{columnValue.unit}</span>}
          </td>
        );
      })}
      <div className="damage-bar" style={{ backgroundColor: color, width: `${skill.percentage}%` }} />
    </tr>
  );
};
