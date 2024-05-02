import { CharacterType, ComputedSkillGroup } from "@/types";
import { getSkillName } from "@/utils";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { SkillRow } from "./SkillRow";
import { useSkillGroupRow } from "./useSkillGroupRow";

export type SkillRowProps = {
  characterType: CharacterType;
  group: ComputedSkillGroup;
  color: string;
};

export const SkillGroupRow = ({ characterType, group, color }: SkillRowProps) => {
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
    expanded,
    setExpanded,
    sortedSkills,
  } = useSkillGroupRow(group);

  return (
    <>
      <tr className="skill-row group" onClick={() => setExpanded(!expanded)}>
        <td className="text-left row-data">
          <span>{getSkillName(group.childCharacterType, group)}</span>
          <span className="p4">{expanded ? <CaretUp size={12} /> : <CaretDown size={12} />}</span>
        </td>
        <td className="text-center row-data">{group.hits}</td>
        <td className="text-center row-data">
          {showFullValues ? (
            group.totalDamage.toLocaleString()
          ) : (
            <>
              {totalDamage}
              <span className="unit font-sm">{totalDamageUnit}</span>
            </>
          )}
        </td>
        <td className="text-center row-data">
          {showFullValues ? (
            group.minDamage ? (
              group.minDamage.toLocaleString()
            ) : (
              ""
            )
          ) : (
            <>
              {group.minDamage && minDmg}
              <span className="unit font-sm">{minDmgUnit}</span>
            </>
          )}
        </td>
        <td className="text-center row-data">
          {showFullValues ? (
            group.maxDamage ? (
              group.maxDamage.toLocaleString()
            ) : (
              ""
            )
          ) : (
            <>
              {group.maxDamage && maxDmg}
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
          {group.percentage.toFixed(0)}
          <span className="unit font-sm">%</span>
        </td>
        <div className="damage-bar" style={{ backgroundColor: color, width: `${group.percentage}%` }} />
      </tr>
      {expanded &&
        sortedSkills.map((skill) => (
          <SkillRow
            key={`${skill.childCharacterType}-${getSkillName(characterType, skill)}`}
            characterType={characterType}
            skill={skill}
            color={color}
            nested
          />
        ))}
    </>
  );
};
