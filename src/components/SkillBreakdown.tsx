import { CharacterType, ComputedPlayerState, ComputedSkillGroup, ComputedSkillState } from "@/types";

import { getSkillName } from "@/utils";
import { SkillGroupRow } from "./SkillGroupRow";
import { SkillRow } from "./SkillRow";
import { useSkillBreakdown } from "./useSkillBreakdown";

export type SkillBreakdownProps = {
  player: ComputedPlayerState;
  color: string;
};

const renderSkillRow = (
  characterType: CharacterType,
  skillData: ComputedSkillState | ComputedSkillGroup,
  color: string
) => {
  const isSkillGroup = typeof skillData.actionType === "object" && Object.hasOwn(skillData.actionType, "Group");

  if (isSkillGroup) {
    const skillGroup = skillData as ComputedSkillGroup;

    return (
      <SkillGroupRow
        key={`${skillGroup.childCharacterType}-${getSkillName(characterType, skillGroup)}`}
        characterType={characterType}
        group={skillGroup}
        color={color}
      />
    );
  } else {
    const skill = skillData as ComputedSkillState;

    return (
      <SkillRow
        key={`${skill.childCharacterType}-${getSkillName(characterType, skill)}`}
        characterType={characterType}
        skill={skill}
        color={color}
      />
    );
  }
};

export const SkillBreakdown = ({ player, color }: SkillBreakdownProps) => {
  const { skills } = useSkillBreakdown(player);

  return (
    <tr className="skill-table">
      <td colSpan={100}>
        <table className="table w-full">
          <thead className="header transparent-bg">
            <tr>
              <th className="header-name">Skill</th>
              <th className="header-column text-center">Hits</th>
              <th className="header-column text-center">Total</th>
              <th className="header-column text-center">Min</th>
              <th className="header-column text-center">Max</th>
              <th className="header-column text-center">Avg</th>
              <th className="header-column text-center">%</th>
            </tr>
          </thead>
          <tbody className="transparent-bg">
            {skills.map((skill) => renderSkillRow(player.characterType, skill, color))}
          </tbody>
        </table>
      </td>
    </tr>
  );
};
