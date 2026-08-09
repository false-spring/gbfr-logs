import { useShallow } from "zustand/react/shallow";

import SkillGroupMapping from "@/assets/skill-groups";
import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, ComputedSbaGroup, ComputedSbaSourceState, SbaCause, SbaSourceState } from "@/types";
import { isAttributedCause } from "@/utils/derive";
import { getSbaCauseName } from "@/utils/i18n";

const groupableActionId = (cause: SbaCause): number | null => {
  if (typeof cause === "string") return null;

  const action = Object.hasOwn(cause, "Action")
    ? (cause as { Action: unknown }).Action
    : (cause as { Inferred: unknown }).Inferred;

  if (typeof action !== "object" || action === null || !Object.hasOwn(action, "Normal")) return null;

  return (action as { Normal: number }).Normal;
};

export const useSbaBreakdown = (player: ComputedPlayerState) => {
  const { useCondensedSkills } = useMeterSettingsStore(
    useShallow((state) => ({
      useCondensedSkills: state.use_condensed_skills,
    }))
  );

  const total = player.totalSbaAdded ?? 0;
  const breakdown = player.sbaBreakdown ?? [];

  // Fold rows that render under the same name (a cause and its inferred counterpart).
  const folded = new Map<string, SbaSourceState>();
  for (const source of breakdown) {
    const key = getSbaCauseName(player.characterType, source.cause, source.childCharacterType);
    const existing = folded.get(key);

    if (existing) {
      existing.ticks += source.ticks;
      existing.totalSbaAdded += source.totalSbaAdded;
    } else {
      folded.set(key, { ...source });
    }
  }

  const computed = [...folded.values()].map<ComputedSbaSourceState>((source) => ({
    ...source,
    percentage: total === 0 ? 0 : (source.totalSbaAdded / total) * 100,
  }));

  const attributed = computed.filter((s) => isAttributedCause(s.cause));
  const attributedGauge = attributed.reduce((sum, s) => sum + s.totalSbaAdded, 0);

  let sources: Array<ComputedSbaSourceState | ComputedSbaGroup> = computed;

  if (useCondensedSkills && typeof player.characterType === "string") {
    const grouped: Array<ComputedSbaSourceState | ComputedSbaGroup> = [];

    for (const source of computed) {
      const actionId = groupableActionId(source.cause);

      const mappingKey = source.childCharacterType ?? player.characterType;
      const mapping = (typeof mappingKey === "string" && SkillGroupMapping[mappingKey]) || {};

      const groupName =
        actionId === null ? undefined : Object.keys(mapping).find((name) => mapping[name].skills.includes(actionId));

      if (groupName === undefined) {
        grouped.push(source);
        continue;
      }

      const existing = grouped.find(
        (row) =>
          typeof row.cause === "object" &&
          Object.hasOwn(row.cause, "Action") &&
          typeof (row.cause as { Action: unknown }).Action === "object" &&
          Object.hasOwn((row.cause as { Action: object }).Action, "Group") &&
          (row.cause as { Action: { Group: string } }).Action.Group === groupName &&
          row.childCharacterType === source.childCharacterType
      ) as ComputedSbaGroup | undefined;

      if (existing) {
        existing.ticks += source.ticks;
        existing.totalSbaAdded += source.totalSbaAdded;
        existing.percentage += source.percentage;
        existing.sources.push(source);
      } else {
        grouped.push({
          cause: { Action: { Group: groupName } },
          childCharacterType: source.childCharacterType,
          ticks: source.ticks,
          totalSbaAdded: source.totalSbaAdded,
          percentage: source.percentage,
          sources: [source],
        });
      }
    }

    sources = grouped;
  }

  sources.sort((a, b) => b.totalSbaAdded - a.totalSbaAdded);

  return {
    sources,
    attributedPercentage: total === 0 ? 0 : (attributedGauge / total) * 100,
    hasUnattributed: computed.length > attributed.length,
  };
};
