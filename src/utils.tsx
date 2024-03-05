import html2canvas from "html2canvas";
import toast from "react-hot-toast";
import { ComputedPlayerData, EncounterState, CharacterType, ComputedSkillState, PlayerData } from "./types";
import { t } from "i18next";

export const formatInPartyOrder = (party: Record<string, PlayerData>): ComputedPlayerData[] => {
  const players = Object.keys(party).map((key) => {
    return party[key];
  });

  players.sort((a, b) => a.index - b.index);

  return players.map((player, i) => ({
    partyIndex: i,
    ...player,
  }));
};

export const epochToLocalTime = (epoch: number): string => {
  const utc = new Date(epoch);

  return new Intl.DateTimeFormat("default", {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
    second: "numeric",
  }).format(utc);
};

export const getSkillName = (characterType: CharacterType, skill: ComputedSkillState) => {
  switch (true) {
    case skill.actionType === "LinkAttack":
      return t([`skills.${characterType}.link-attack`, "skills.default.link-attack"]);
    case skill.actionType === "SBA":
      return t([`skills.${characterType}.skybound-arts`, "skills.default.skybound-arts"]);
    case typeof skill.actionType == "object" && Object.hasOwn(skill.actionType, "SupplementaryDamage"):
      return t(["skills.default.supplementary-damage"]);
    case typeof skill.actionType == "object" && Object.hasOwn(skill.actionType, "DamageOverTime"):
      return t([
        `skills.${skill.childCharacterType}.damage-over-time`,
        `skills.${characterType}.damage-over-time`,
        "skills.default.damage-over-time",
      ]);
    case typeof skill.actionType == "object" && Object.hasOwn(skill.actionType, "Normal"): {
      const actionType = skill.actionType as { Normal: number };
      const skillID = actionType["Normal"];

      return t(
        [
          `skills.${skill.childCharacterType}.${skillID}`,
          `skills.${characterType}.${skillID}`,
          `skills.default.${skillID}`,
          `skills.default.unknown-skill`,
        ],
        { id: skillID }
      );
    }
    default:
      return t("ui.unknown");
  }
};
const tryParseInt = (intString: string | number, defaultValue = 0) => {
  if (typeof intString === "number") {
    if (isNaN(intString)) return defaultValue;
    return intString;
  }

  let intNum;

  try {
    intNum = parseInt(intString);
    if (isNaN(intNum)) intNum = defaultValue;
  } catch {
    intNum = defaultValue;
  }

  return intNum;
};

// Takes a number and returns a shortened version of it that is friendlier to read.
// For example, 1200 would be returned as 1.2k, 1200000 as 1.2m, and so on.
export const humanizeNumbers = (n: number) => {
  if (n >= 1e3 && n < 1e6) return [+(n / 1e3).toFixed(1), "k"];
  if (n >= 1e6 && n < 1e9) return [+(n / 1e6).toFixed(1), "m"];
  if (n >= 1e9 && n < 1e12) return [+(n / 1e9).toFixed(1), "b"];
  if (n >= 1e12) return [+(n / 1e12).toFixed(1), "t"];
  else return [tryParseInt(n).toFixed(0), ""];
};

export const millisecondsToElapsedFormat = (ms: number): string => {
  const date = new Date(Date.UTC(0, 0, 0, 0, 0, 0, ms));
  return `${date.getUTCMinutes().toString().padStart(2, "0")}:${date.getUTCSeconds().toString().padStart(2, "0")}`;
};

export const exportScreenshotToClipboard = () => {
  const app = document.querySelector(".app") as HTMLElement;

  html2canvas(app, {
    backgroundColor: "#252525",
  }).then((canvas) => {
    canvas.toBlob((blob) => {
      if (blob) {
        const item = new ClipboardItem({ "image/png": blob });
        navigator.clipboard.write([item]).then(() => {
          toast.success("Screenshot copied to clipboard!");
        });
      }
    });
  });
};

export const translatedPlayerName = (player: ComputedPlayerData) =>
  `[${player.partyIndex + 1}]` + " " + t(`characters.${player.characterType}`);

export const exportEncounterToClipboard = (encounterState: EncounterState) => {
  const playerHeader = `Name,DMG,DPS,%`;
  const orderedPlayers = formatInPartyOrder(encounterState.party);
  const players: Array<ComputedPlayerData> = orderedPlayers.map((playerData) => {
    return {
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
      ...playerData,
    };
  });

  players.sort((a, b) => b.totalDamage - a.totalDamage);

  const playerData = players
    .map((player) => {
      const totalDamage = player.skills.reduce((acc, skill) => acc + skill.totalDamage, 0);
      const computedSkills = player.skills.map((skill) => {
        return {
          percentage: (skill.totalDamage / totalDamage) * 100,
          ...skill,
        };
      });

      computedSkills.sort((a, b) => b.totalDamage - a.totalDamage);

      const playerLine = [
        translatedPlayerName(player),
        player.totalDamage,
        Math.round(player.dps),
        player.percentage,
      ].join(",");

      const skillHeader = ["Skill", "Hits", "Total", "Min", "Max", "Avg", "%"].join(",");

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
            skill.percentage.toFixed(2),
          ].join(",");
        })
        .join("\n");

      return [playerHeader, playerLine, skillHeader, skillLine].join("\n");
    })
    .join("\n\n");

  navigator.clipboard.writeText(playerData).then(() => {
    toast.success("Copied text to clipboard!");
  });
};

export const PLAYER_COLORS = ["#FF5630", "#FFAB00", "#36B37E", "#00B8D9", "#9BCF53", "#380E7F", "#416D19", "#2C568D"];
