import { useShallow } from "zustand/react/shallow";

import SkillGroupMapping from "@/assets/skill-groups";
import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, ComputedSkillGroup, ComputedSkillState } from "@/types";
import { getSkillName } from "@/utils";

export const useSkillBreakdown = (player: ComputedPlayerState) => {
  const { useCondensedSkills } = useMeterSettingsStore(
    useShallow((state) => ({
      useCondensedSkills: state.use_condensed_skills,
    }))
  );

  const totalDamage = player.skillBreakdown.reduce((acc, skill) => acc + skill.totalDamage, 0);
  const computedSkills = player.skillBreakdown.map<ComputedSkillState>((skill) => {
    return {
      percentage: (skill.totalDamage / totalDamage) * 100,
      groupName: getSkillName(player.characterType, skill),
      ...skill,
    };
  });

  let skillsToShow: Array<ComputedSkillGroup | ComputedSkillState> = computedSkills;

  if (useCondensedSkills && typeof player.characterType == "string") {
    const skills: Array<ComputedSkillGroup | ComputedSkillGroup> = [];

    for (const skill of computedSkills) {
      const skillGroupIndex = typeof skill.childCharacterType !== "string" ? -1 : skill.childCharacterType;
      const skillGroupMapping = SkillGroupMapping[skillGroupIndex] || {};

      if (typeof skill.actionType == "object" && Object.hasOwn(skill.actionType, "Normal")) {
        const actionType = skill.actionType as { Normal: number };
        let wasGroupedSkill = false;

        for (const group in skillGroupMapping) {
          const groupActionType = { Group: group };
          const skillBelongsToGroup = skillGroupMapping[group].skills.includes(actionType.Normal);

          if (skillBelongsToGroup) {
            const skillGroupIndex = skills.findIndex((skillGroup) => {
              if (typeof skillGroup.actionType === "object" && Object.hasOwn(skillGroup.actionType, "Group")) {
                const actionType = skillGroup.actionType as { Group: string };

                return actionType.Group == group && skillGroup.childCharacterType == skill.childCharacterType;
              } else {
                return false;
              }
            });

            if (skillGroupIndex >= 0) {
              const skillGroup = skills[skillGroupIndex] as ComputedSkillGroup;

              skills[skillGroupIndex] = {
                ...skillGroup,
                hits: skillGroup.hits + skill.hits,
                percentage: skillGroup.percentage + skill.percentage,
                totalDamage: skillGroup.totalDamage + skill.totalDamage,
                minDamage: Math.min(skillGroup?.minDamage || 0, skill.minDamage || 0),
                maxDamage: Math.max(skillGroup?.maxDamage ?? Number.MIN_VALUE, skill.maxDamage || 0),
                skills: [...(skillGroup.skills || []), skill],
              };
            } else {
              skills.push({
                actionType: groupActionType,
                childCharacterType: skill.childCharacterType,
                hits: skill.hits,
                totalDamage: skill.totalDamage,
                minDamage: skill.minDamage,
                maxDamage: skill.maxDamage,
                percentage: skill.percentage,
                skills: [skill],
              });
            }

            wasGroupedSkill = true;

            break;
          }
        }

        if (!wasGroupedSkill) {
          skills.push(skill);
        }
      } else {
        skills.push(skill);
      }
    }

    skillsToShow = skills;
  }

  skillsToShow.sort((a, b) => b.totalDamage - a.totalDamage);

  return {
    skills: skillsToShow,
  };
};
