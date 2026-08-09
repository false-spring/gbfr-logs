import { CharacterType, ComputedPlayerState, ComputedSkillGroup, ComputedSkillState, SkillMeterColumns } from "@/types";

import { skillRowKey } from "@/utils/derive";
import { SbaBreakdown } from "./SbaBreakdown";
import { SkillGroupRow } from "./SkillGroupRow";
import { SkillRow } from "./SkillRow";
import { DAMAGE_SKILL_COLUMNS, SKILL_COLUMN_LABEL, STUN_SKILL_COLUMNS } from "./skillColumns";
import { useSkillBreakdown } from "./useSkillBreakdown";

export type SkillBreakdownProps = {
  player: ComputedPlayerState;
  color: string;
  metric?: "damage" | "stun" | "sba";
};

const renderSkillRow = (
  characterType: CharacterType,
  skillData: ComputedSkillState | ComputedSkillGroup,
  color: string,
  columns: SkillMeterColumns[],
  stunPerSecondRatio: number
) => {
  const isSkillGroup = typeof skillData.actionType === "object" && Object.hasOwn(skillData.actionType, "Group");

  if (isSkillGroup) {
    const skillGroup = skillData as ComputedSkillGroup;

    return (
      <SkillGroupRow
        key={skillRowKey(skillGroup)}
        characterType={characterType}
        group={skillGroup}
        color={color}
        columns={columns}
        stunPerSecondRatio={stunPerSecondRatio}
      />
    );
  } else {
    const skill = skillData as ComputedSkillState;

    return (
      <SkillRow
        key={skillRowKey(skill)}
        characterType={characterType}
        skill={skill}
        color={color}
        columns={columns}
        stunPerSecondRatio={stunPerSecondRatio}
      />
    );
  }
};

export const SkillBreakdown = ({ player, color, metric = "damage" }: SkillBreakdownProps) => {
  if (metric === "sba") {
    return <SbaBreakdown player={player} color={color} />;
  }

  return <DamageOrStunBreakdown player={player} color={color} metric={metric} />;
};

const DamageOrStunBreakdown = ({
  player,
  color,
  metric,
}: {
  player: ComputedPlayerState;
  color: string;
  metric: "damage" | "stun";
}) => {
  const { skills } = useSkillBreakdown(player, metric);
  const columns = metric === "stun" ? STUN_SKILL_COLUMNS : DAMAGE_SKILL_COLUMNS;
  const stunPerSecondRatio = player.totalStunValue > 0 ? player.stunPerSecond / player.totalStunValue : 0;

  return (
    <tr className="skill-table">
      <td colSpan={100}>
        <table className="table w-full">
          <thead className="header transparent-bg">
            <tr>
              <th className="header-name">Skill</th>
              {columns.map((column) => (
                <th key={column} className="header-column text-center">
                  {SKILL_COLUMN_LABEL[column]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="transparent-bg">
            {skills.map((skill) => renderSkillRow(player.characterType, skill, color, columns, stunPerSecondRatio))}
          </tbody>
        </table>
      </td>
    </tr>
  );
};
