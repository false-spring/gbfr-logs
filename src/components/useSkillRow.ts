import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedSkillState, SkillMeterColumns } from "@/types";
import { useShallow } from "zustand/react/shallow";
import { getSkillColumnValue } from "./skillColumns";

export const useSkillRow = (skill: ComputedSkillState, stunPerSecondRatio: number) => {
  const { show_full_values } = useMeterSettingsStore(
    useShallow((state) => ({
      show_full_values: state.show_full_values,
    }))
  );

  return {
    showFullValues: show_full_values,
    getColumnValue: (column: SkillMeterColumns) =>
      getSkillColumnValue(column, skill, show_full_values, stunPerSecondRatio),
  };
};
