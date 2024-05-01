import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedSkillGroup } from "@/types";
import { humanizeNumbers } from "@/utils";
import { useState } from "react";
import { useShallow } from "zustand/react/shallow";

export const useSkillGroupRow = (group: ComputedSkillGroup) => {
  const { show_full_values } = useMeterSettingsStore(
    useShallow((state) => ({
      show_full_values: state.show_full_values,
    }))
  );

  const [expanded, setExpanded] = useState(false);

  const [totalDamage, totalDamageUnit] = humanizeNumbers(group.totalDamage);
  const [minDmg, minDmgUnit] = humanizeNumbers(group.minDamage || 0);
  const [maxDmg, maxDmgUnit] = humanizeNumbers(group.maxDamage || 0);
  const rawAverageDmg = group.hits === 0 ? 0 : group.totalDamage / group.hits;
  const [averageDmg, averageDmgUnit] = humanizeNumbers(rawAverageDmg);

  const sortedSkills = (group.skills || []).sort((a, b) => b.totalDamage - a.totalDamage);

  return {
    showFullValues: show_full_values,
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
  };
};
