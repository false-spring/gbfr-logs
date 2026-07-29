import { useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { ComputedPlayerState, MeterColumns, PlayerData } from "@/types";
import { playerStunPerHit, resolvePartySlotIndex } from "@/utils/derive";
import { humanizeNumbers } from "@/utils/format";
import { METER_DAMAGE_COLUMNS, METER_SBA_COLUMNS, METER_STUN_COLUMNS } from "./skillColumns";

export type ColumnValue = {
  value: string | number;
  unit?: string | number;
};

export const usePlayerRow = (
  live: boolean,
  player: ComputedPlayerState,
  partyData: Array<PlayerData | null>,
  partyTotalStunValue: number,
  metric: "damage" | "stun" | "sba" = "damage"
) => {
  const { color_1, color_2, color_3, color_4, show_display_names, show_full_values, overlay_columns } =
    useMeterSettingsStore(
      useShallow((state) => ({
        color_1: state.color_1,
        color_2: state.color_2,
        color_3: state.color_3,
        color_4: state.color_4,
        show_display_names: state.show_display_names,
        show_full_values: state.show_full_values,
        overlay_columns: state.overlay_columns,
      }))
    );

  const [isOpen, setIsOpen] = useState(false);

  const playerColors = [color_1, color_2, color_3, color_4, "#9BCF53", "#380E7F", "#416D19", "#2C568D"];
  const partySlotIndex = resolvePartySlotIndex(partyData, player.index);
  const color = partySlotIndex !== -1 ? playerColors[partySlotIndex] : playerColors[player.partyIndex];

  const [totalDamage, totalDamageUnit] = humanizeNumbers(player.totalDamage);
  const [dps, dpsUnit] = humanizeNumbers(player.dps);
  const [totalStunValue, totalStunValueUnit] = humanizeNumbers(player.totalStunValue);
  const [healDone, healDoneUnit] = humanizeNumbers(player.healDone);
  const perHitStun = playerStunPerHit(player);
  const [perHitStunValue, perHitStunUnit] = humanizeNumbers(perHitStun);

  // Function for matching the column type to the value to display in the table.
  const matchColumnTypeToValue = (showFullValues: boolean, column: MeterColumns): ColumnValue => {
    switch (column) {
      case MeterColumns.TotalDamage:
        return showFullValues
          ? { value: (player.totalDamage || 0).toLocaleString() }
          : { value: totalDamage, unit: totalDamageUnit };
      case MeterColumns.DPS:
        return showFullValues ? { value: (player.dps || 0).toLocaleString() } : { value: dps, unit: dpsUnit };
      case MeterColumns.DamagePercentage:
        return { value: (player.percentage || 0).toFixed(0), unit: "%" };
      case MeterColumns.SBA:
        return showFullValues
          ? { value: (player.sba / 10).toFixed(2) }
          : { value: (player.sba / 10).toFixed(2), unit: "%" };
      case MeterColumns.StunPerSecond:
        return { value: (player.stunPerSecond || 0).toLocaleString() };
      case MeterColumns.TotalStunValue:
        return showFullValues
          ? { value: (player.totalStunValue || 0).toLocaleString() }
          : { value: totalStunValue, unit: totalStunValueUnit };
      case MeterColumns.StunPerHit:
        return showFullValues
          ? { value: (perHitStun || 0).toLocaleString() }
          : { value: perHitStunValue, unit: perHitStunUnit };
      case MeterColumns.StunPercentage: {
        const stunShare = partyTotalStunValue > 0 ? (player.totalStunValue / partyTotalStunValue) * 100 : 0;
        return { value: stunShare.toFixed(0), unit: "%" };
      }
      case MeterColumns.HealDone:
        return showFullValues
          ? { value: (player.healDone || 0).toLocaleString() }
          : { value: healDone, unit: healDoneUnit };
      case MeterColumns.TotalSbaAdded:
        return { value: ((player.totalSbaAdded ?? 0) / 10).toFixed(1), unit: "%" };
      case MeterColumns.SbaPercentage:
        return { value: (player.percentage || 0).toFixed(0), unit: "%" };
      default:
        return { value: "" };
    }
  };

  const columns = live
    ? overlay_columns
    : metric === "sba"
      ? METER_SBA_COLUMNS
      : metric === "stun"
        ? METER_STUN_COLUMNS
        : METER_DAMAGE_COLUMNS;

  return {
    columns,
    isOpen,
    setIsOpen,
    color,
    matchColumnTypeToValue,
    partySlotIndex,
    showFullValues: show_full_values,
    showDisplayNames: show_display_names,
  };
};
