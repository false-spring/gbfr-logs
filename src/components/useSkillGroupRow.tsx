import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedSkillGroup, SkillMeterColumns } from "@/types";
import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { getSkillColumnValue } from "./skillColumns";

export const useSkillGroupRow = (
  group: ComputedSkillGroup,
  columns: SkillMeterColumns[],
  stunPerSecondRatio: number
) => {
  const { show_full_values } = useMeterSettingsStore(
    useShallow((state) => ({
      show_full_values: state.show_full_values,
    }))
  );

  const [expanded, setExpanded] = useState(false);

  const sortedSkills = columns.includes(SkillMeterColumns.TotalStunValue)
    ? (group.skills || []).sort((a, b) => b.totalStunValue - a.totalStunValue)
    : (group.skills || []).sort((a, b) => b.totalDamage - a.totalDamage);

  return {
    showFullValues: show_full_values,
    getColumnValue: (column: SkillMeterColumns) =>
      getSkillColumnValue(column, group, show_full_values, stunPerSecondRatio),
    expanded,
    setExpanded,
    sortedSkills,
  };
};
