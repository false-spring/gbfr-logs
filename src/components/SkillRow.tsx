import { CharacterType, ComputedSkillState } from "@/types";
import { getSkillName } from "@/utils";
import { useSkillRow } from "./useSkillRow";

export type SkillRowProps = {
  characterType: CharacterType;
  skill: ComputedSkillState;
  color: string;
  nested?: boolean;
};

export const SkillRow = ({ characterType, skill, color, nested }: SkillRowProps) => {
  const {
    showFullValues,
    totalDamage,
    totalDamageUnit,
    minDmg,
    minDmgUnit,
    maxDmg,
    maxDmgUnit,
    rawAverageDmg,
    averageDmg,
    averageDmgUnit,
  } = useSkillRow(skill);

  return (
    <tr className={`skill-row ${nested ? "nested" : ""}`}>
      {nested ? (
        <td className="text-left row-data nested">{getSkillName(characterType, skill)}</td>
      ) : (
        <td className="text-left row-data">{getSkillName(characterType, skill)}</td>
      )}
      <td className="text-center row-data">{skill.hits}</td>
      <td className="text-center row-data">
        {showFullValues ? (
          skill.totalDamage.toLocaleString()
        ) : (
          <>
            {totalDamage}
            <span className="unit font-sm">{totalDamageUnit}</span>
          </>
        )}
      </td>
      <td className="text-center row-data">
        {showFullValues ? (
          skill.minDamage ? (
            skill.minDamage.toLocaleString()
          ) : (
            ""
          )
        ) : (
          <>
            {skill.minDamage && minDmg}
            <span className="unit font-sm">{minDmgUnit}</span>
          </>
        )}
      </td>
      <td className="text-center row-data">
        {showFullValues ? (
          skill.maxDamage ? (
            skill.maxDamage.toLocaleString()
          ) : (
            ""
          )
        ) : (
          <>
            {skill.maxDamage && maxDmg}
            <span className="unit font-sm">{maxDmgUnit}</span>
          </>
        )}
      </td>
      <td className="text-center row-data">
        {showFullValues ? (
          rawAverageDmg.toLocaleString()
        ) : (
          <>
            {averageDmg}
            <span className="unit font-sm">{averageDmgUnit}</span>
          </>
        )}
      </td>
      <td className="text-center row-data">
        {skill.percentage.toFixed(0)}
        <span className="unit font-sm">%</span>
      </td>
      <div className="damage-bar" style={{ backgroundColor: color, width: `${skill.percentage}%` }} />
    </tr>
  );
};
