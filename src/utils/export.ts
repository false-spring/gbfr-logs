import { open } from "@tauri-apps/api/shell";
import html2canvas from "html2canvas";
import * as jsurl from "jsurl";
import toast from "react-hot-toast";

import { t } from "i18next";

import { ComputedPlayerState, LiveEncounterState, PlayerData, SortDirection, SortType } from "@/types";
import { formatInPartyOrder, resolvePartySlotIndex, sortPlayers } from "@/utils/derive";
import { millisecondsToElapsedFormat } from "@/utils/format";
import { getSkillName, translatedPlayerName } from "@/utils/i18n";

/// Captures a screenshot of the meter and copies it to the clipboard.
export const exportScreenshotToClipboard = (selector = ".app") => {
  const app = document.querySelector(selector) as HTMLElement;

  html2canvas(app, {
    backgroundColor: "#252525",
  }).then((canvas) => {
    canvas.toBlob((blob) => {
      if (blob) {
        const item = new ClipboardItem({ "image/png": blob });
        navigator.clipboard.write([item]).then(() => {
          toast.success(t("ui.copied-to-clipboard"));
        });
      }
    });
  });
};

/// Exports the character data to the clipboard in a detailed format (JSON)
export const exportCharacterDataToClipboard = (playerData: PlayerData, showDisplayNames: boolean) => {
  const ct = playerData.characterType;
  const exported: Partial<PlayerData> = {
    ...playerData,
    displayName: showDisplayNames ? playerData.displayName : "",
    characterName:
      typeof ct === "string"
        ? t(`characters:${ct}`, `ui:characters.${ct}`)
        : `#${ct.Unknown.toString(16).padStart(8, "0").toUpperCase()}`,
  };
  delete exported.networkUserId;
  delete exported.networkUserName;

  navigator.clipboard.writeText(JSON.stringify(exported)).then(() => {
    toast.success(t("ui.copied-to-clipboard"));
  });
};

/// Exports the character data to the the Relink Damage Calculator application.
export const openDamageCalculator = (playerData: PlayerData) => {
  const exported: Partial<PlayerData> = { ...playerData };
  delete exported.networkUserId;
  delete exported.networkUserName;

  const data = jsurl.stringify(exported);

  open(`https://relink-damage.vercel.app/?logsdata=${data}`);
};

/// Exports the encounter data to the clipboard in a simple format (CSV)
export const exportSimpleEncounterToClipboard = (
  sortType: SortType,
  sortDirection: SortDirection,
  encounterState: LiveEncounterState,
  partyData: Array<PlayerData | null>,
  showDisplayNames: boolean
) => {
  if (encounterState.totalDamage === 0) return toast.error("Nothing to copy!");

  const encounterHeader = "Encounter Time, Total Damage, Total DPS";
  const encounterValues = [
    millisecondsToElapsedFormat(encounterState.endTime - encounterState.startTime),
    encounterState.totalDamage,
    Math.round(encounterState.dps),
  ].join(", ");

  const encounterData = [encounterHeader, encounterValues].join("\n");

  const orderedPlayers = formatInPartyOrder(encounterState.party);

  const players: Array<ComputedPlayerState> = orderedPlayers.map((playerData) => {
    return {
      ...playerData,
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
    };
  });

  sortPlayers(players, sortType, sortDirection);

  const playerHeader = "Name, DMG, DPS, %";
  const playerData = players
    .map((player) => {
      const totalDamage = player.skillBreakdown.reduce((acc, skill) => acc + skill.totalDamage, 0);
      const computedSkills = player.skillBreakdown.map((skill) => {
        return {
          percentage: (skill.totalDamage / totalDamage) * 100,
          ...skill,
        };
      });

      computedSkills.sort((a, b) => b.totalDamage - a.totalDamage);

      const partySlotIndex = resolvePartySlotIndex(partyData, player.index);

      return [
        translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player, showDisplayNames),
        player.totalDamage,
        Math.round(player.dps),
        `${player.percentage?.toFixed(2)}%`,
      ].join(", ");
    })
    .join("\n");

  navigator.clipboard.writeText([encounterData, playerHeader, playerData].join("\n")).then(() => {
    toast.success(t("ui.copied-to-clipboard"));
  });
};

/// Exports the encounter data to the clipboard in a detailed format (CSV)
export const exportFullEncounterToClipboard = (
  sortType: SortType,
  sortDirection: SortDirection,
  encounterState: LiveEncounterState,
  partyData: Array<PlayerData | null>,
  showDisplayNames: boolean
) => {
  if (encounterState.totalDamage === 0) return toast.error("Nothing to copy!");

  const encounterHeader = "Encounter Time, Total Damage, Total DPS";
  const encounterValues = [
    millisecondsToElapsedFormat(encounterState.endTime - encounterState.startTime),
    encounterState.totalDamage,
    Math.round(encounterState.dps),
  ].join(", ");

  const encounterData = [encounterHeader, encounterValues].join("\n");

  const playerHeader = "Name, DMG, DPS, %";
  const orderedPlayers = formatInPartyOrder(encounterState.party);

  const players: Array<ComputedPlayerState> = orderedPlayers.map((playerData) => {
    return {
      ...playerData,
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
    };
  });

  sortPlayers(players, sortType, sortDirection);

  const playerData = players
    .map((player) => {
      const totalDamage = player.skillBreakdown.reduce((acc, skill) => acc + skill.totalDamage, 0);
      const computedSkills = player.skillBreakdown.map((skill) => {
        return {
          percentage: (skill.totalDamage / totalDamage) * 100,
          ...skill,
        };
      });

      const partySlotIndex = resolvePartySlotIndex(partyData, player.index);

      computedSkills.sort((a, b) => b.totalDamage - a.totalDamage);

      const playerLine = [
        translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player, showDisplayNames),
        player.totalDamage,
        Math.round(player.dps),
        `${player.percentage?.toFixed(2)}%`,
      ].join(", ");

      const skillHeader = ["Skill", "Hits", "Total", "Min", "Max", "Avg", "%"].join(", ");

      const skillLine = computedSkills
        .map((skill) => {
          const skillName = getSkillName(player.characterType, skill);
          const averageHit = skill.hits === 0 ? 0 : skill.totalDamage / skill.hits;

          return [
            skillName,
            skill.hits,
            skill.totalDamage,
            skill.minDamage,
            skill.maxDamage,
            Math.round(averageHit),
            `${skill.percentage.toFixed(2)}%`,
          ].join(", ");
        })
        .join("\n");

      return [playerHeader, playerLine, skillHeader, skillLine].join("\n");
    })
    .join("\n");

  navigator.clipboard.writeText([encounterData, playerData].join("\n")).then(() => {
    toast.success(t("ui.copied-to-clipboard"));
  });
};
