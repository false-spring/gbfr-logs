import { useShallow } from "zustand/react/shallow";

import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, ComputedSkillState } from "@/types";
import { getSkillName } from "@/utils";

export const useSkillBreakdown = (player: ComputedPlayerState) => {
  const { useCondensedSkills } = useMeterSettingsStore(
    useShallow((state) => ({
      useCondensedSkills: state.use_condensed_skills,
    }))
  );

  const totalDamage = player.skillBreakdown.reduce((acc, skill) => acc + skill.totalDamage, 0);
  const computedSkills = player.skillBreakdown.map((skill) => {
    return {
      percentage: (skill.totalDamage / totalDamage) * 100,
      groupName: getSkillName(player.characterType, skill),
      ...skill,
    };
  });

  let skillsToShow = computedSkills;

  if (useCondensedSkills) {
    const mergedSkillMap: Map<string, ComputedSkillState> = new Map();
    const matchRegex = /(.+?)(Lvl [0-9]+|[0-9]|\().*/; // Will match "Attack 1" and "Attack 2" to just "Attack ". Assumes skill names won't have numbers in them otherwise
    const groupingFn = (skillName: string) => (skillName.match(matchRegex)?.[1] ?? skillName).trim();

    computedSkills.forEach((skill) => {
      const shortName = groupingFn(skill.groupName);
      const existing: ComputedSkillState | undefined = mergedSkillMap.get(shortName);
      mergedSkillMap.set(shortName, {
        groupName: shortName,
        minDamage: Math.min(existing?.totalDamage ?? Number.MAX_VALUE, skill.minDamage ?? Number.MAX_VALUE),
        maxDamage: Math.max(existing?.totalDamage ?? Number.MIN_VALUE, skill.maxDamage ?? Number.MIN_VALUE),
        hits: (existing?.hits ?? 0) + skill.hits,
        totalDamage: (existing?.totalDamage ?? 0) + skill.totalDamage,
        percentage: (existing?.percentage ?? 0) + skill.percentage,

        // Just take the first childCharacterType and actionType since there is no good way to merge them
        childCharacterType: existing?.childCharacterType ?? skill.childCharacterType,
        actionType: existing?.actionType ?? skill.actionType,
      });
    });

    skillsToShow = [...mergedSkillMap.values()];
  }

  skillsToShow.sort((a, b) => b.totalDamage - a.totalDamage);

  return {
    skills: skillsToShow,
  };
};
