import html2canvas from "html2canvas";
import toast from "react-hot-toast";
import {
  CharacterType,
  ComputedPlayerState,
  ComputedSkillState,
  EncounterState,
  EnemyType,
  PlayerData,
  PlayerState,
  SortDirection,
  SortType,
} from "./types";

import { t } from "i18next";
import { useEffect, useRef } from "react";

export const EMPTY_ID = 2289754288;

export const formatInPartyOrder = (party: Record<string, PlayerState>): ComputedPlayerState[] => {
  const players = Object.keys(party).map((key) => {
    return party[key];
  });

  players.sort((a, b) => a.index - b.index);

  return players.map((player, i) => ({
    partyIndex: i,
    percentage: 0,
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

/// Takes a number and returns a shortened version of it that is friendlier to read.
/// For example, 1200 would be returned as 1.2k, 1200000 as 1.2m, and so on.
export const humanizeNumbers = (n: number) => {
  if (n >= 1e3 && n < 1e6) return [+(n / 1e3).toFixed(1), "k"];
  if (n >= 1e6 && n < 1e9) return [+(n / 1e6).toFixed(1), "m"];
  if (n >= 1e9 && n < 1e12) return [+(n / 1e9).toFixed(1), "b"];
  if (n >= 1e12) return [+(n / 1e12).toFixed(1), "t"];
  else return [tryParseInt(n).toFixed(0), ""];
};

/// Takes a number of milliseconds and returns a string in the format of MM:SS.
export const millisecondsToElapsedFormat = (ms: number): string => {
  const date = new Date(Date.UTC(0, 0, 0, 0, 0, 0, ms));
  return `${date.getUTCMinutes().toString().padStart(2, "0")}:${date.getUTCSeconds().toString().padStart(2, "0")}`;
};

/// Captures a screenshot of the meter and copies it to the clipboard.
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

/// Formats the player name and translates the player's character type.
export const translatedPlayerName = (
  partySlotIndex: number,
  partySlotData: PlayerData | null,
  player: ComputedPlayerState,
  show_display_names: boolean = true
) => {
  const characterType = t(`characters:${player.characterType}`, `ui:characters.${player.characterType}`);
  const displayName = `${partySlotData?.displayName} (${characterType})`;
  const name = show_display_names && partySlotData?.displayName ? displayName : characterType;

  return `[${partySlotData ? partySlotIndex + 1 : "Guest"}]` + " " + name;
};

export const sortPlayers = (players: ComputedPlayerState[], sortType: SortType, sortDirection: SortDirection) => {
  players.sort((a, b) => {
    if (sortType === "partyIndex") {
      return sortDirection === "asc" ? a.partyIndex - b.partyIndex : b.partyIndex - a.partyIndex;
    } else if (sortType === "dps") {
      return sortDirection === "asc" ? a.dps - b.dps : b.dps - a.dps;
    } else if (sortType === "damage") {
      return sortDirection === "asc" ? a.totalDamage - b.totalDamage : b.totalDamage - a.totalDamage;
    } else if (sortType === "percentage") {
      return sortDirection === "asc" ? a?.percentage - b?.percentage : b?.percentage - a?.percentage;
    }

    return 0;
  });
};

/// Exports the encounter data to the clipboard in a simple format (CSV)
export const exportSimpleEncounterToClipboard = (
  sortType: SortType,
  sortDirection: SortDirection,
  encounterState: EncounterState,
  partyData: Array<PlayerData | null>
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

      const partySlotIndex = partyData.findIndex((partyMember) => partyMember?.actorIndex === player.index);

      return [
        translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player),
        player.totalDamage,
        Math.round(player.dps),
        `${player.percentage?.toFixed(2)}%`,
      ].join(", ");
    })
    .join("\n");

  navigator.clipboard.writeText([encounterData, playerHeader, playerData].join("\n")).then(() => {
    toast.success("Copied text to clipboard!");
  });
};

/// Exports the encounter data to the clipboard in a detailed format (CSV)
export const exportFullEncounterToClipboard = (
  sortType: SortType,
  sortDirection: SortDirection,
  encounterState: EncounterState,
  partyData: Array<PlayerData | null>
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

      const partySlotIndex = partyData.findIndex((partyMember) => partyMember?.actorIndex === player.index);

      computedSkills.sort((a, b) => b.totalDamage - a.totalDamage);

      const playerLine = [
        translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player),
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
    toast.success("Copied text to clipboard!");
  });
};

export const PLAYER_COLORS = ["#FF5630", "#FFAB00", "#36B37E", "#00B8D9", "#9BCF53", "#380E7F", "#416D19", "#2C568D"];

/// Translates the enemy type to a human-readable string.
export const translateEnemyType = (type: EnemyType | null): string => {
  if (type === null) return "";

  if (typeof type == "object" && Object.hasOwn(type, "Unknown")) {
    const hash = type.Unknown.toString(16).padStart(8, "0");

    return t([`enemies:${hash}.text`, `enemies.unknown.${hash}`, "enemies.unknown-type"], { id: hash });
  } else {
    return t([`enemies.${type}`, "enemies.unknown-type"]);
  }
};

export const translateEnemyTypeId = (id: number): string => {
  const hash = toHashString(id);
  return t([`enemies:${hash}.text`, `enemies.unknown.${hash}`, "enemies.unknown-type"], { id: hash });
};

/// Translates the quest ID to a human-readable string.
export const translateQuestId = (id: number | null): string => {
  if (id === null) return "";
  const hash = id.toString(16);
  return t([`quests:${hash}.text`, "quest.unknown"], { id: hash });
};

/// Translates the trait ID to a human-readable string.
export const translateTraitId = (id: number | null): string => {
  if (id === null) return "";
  if (id === EMPTY_ID) return "";

  const hash = id.toString(16).padStart(8, "0");
  return t([`traits:${hash}.text`, "ui.unknown"], { id: hash });
};

/// Translates the sigil ID to a human-readable string.
export const translateSigilId = (id: number | null): string => {
  if (id === null) return "";
  if (id === EMPTY_ID) return "";

  const hash = id.toString(16).padStart(8, "0");
  return t([`sigils:${hash}.text`, "ui.unknown"], { id: hash });
};

/// Translates the item ID to a human-readable string.
export const translateItemId = (id: number | null): string => {
  if (id === null) return "";
  if (id === EMPTY_ID) return "";

  const hash = id.toString(16).padStart(8, "0");
  return t([`items:${hash}.text`, "ui.unknown"], { id: hash });
};

/// Translates the overmastery ID to a human-readable string.
export const translateOvermasteryId = (id: number | null): string => {
  if (id === null) return "";
  if (id === EMPTY_ID) return "";

  const hash = id.toString(16).padStart(8, "0");

  return t([`overmasteries:${hash}.text`, "ui.unknown"], { id: hash });
};

/// Converts a number to a hexadecimal string.
export const toHash = (num: number): string => num.toString(16);

/// Converts a number to a hexadecimal string and pads it to 8 characters.
export const toHashString = (num: number | undefined): string => (num ? num.toString(16).padStart(8, "0") : "");

/// Hook that returns the previous value of a variable.
export const usePrevious = <T>(value: T): T | undefined => {
  const ref = useRef<T>();

  useEffect(() => {
    ref.current = value;
  });

  return ref.current;
};

export const checkCheating = (player: PlayerData) => {
  const cheats = [];

  // invalid wrightstone level
  if ((player.weaponInfo?.trait1Level ?? 0) > 10) {
    cheats.push("Wrightstone with trait level > 10");
  }
  if ((player.weaponInfo?.trait2Level ?? 0) > 7) {
    cheats.push("Wrightstone with trait level > 7");
  }
  if ((player.weaponInfo?.trait3Level ?? 0) > 5) {
    cheats.push("Wrightstone with trait level > 5");
  }

  // invalid wrightstone trait
  const notAllowedWrightstone = [
    "57ab5b10",
    "82ce278d",
    "1568e0e4",
    "70395731",
    "cd18a77d",
    "333e5862",
    "a8a3163b",
    "ec1c6779",
    "dbe1d775",
    "8d2adb6e",
    "5c862e13",
    "082033cb",
    "1b0d9897",
    "9ad8b5e6",
    "40223c28",
    "74aa75d6",
    "dc225c96",
    "4c588c27",
    "5e422ae5",
    "af794a87",
    "57ab5b10",
  ];

  const hasCheatedWrightStone =
    notAllowedWrightstone.includes(toHashString(player.weaponInfo?.trait1Id ?? 0)) ||
    notAllowedWrightstone.includes(toHashString(player.weaponInfo?.trait2Id ?? 0)) ||
    notAllowedWrightstone.includes(toHashString(player.weaponInfo?.trait3Id ?? 0));

  if (hasCheatedWrightStone) {
    cheats.push("Wrightstone with invalid trait");
  }

  // check sigils
  for (const sigil of player.sigils) {
    if (sigil.firstTraitLevel > 15 || sigil.secondTraitLevel > 15 || sigil.sigilLevel > 15) {
      cheats.push(`Modified sigil: over level 15`);
    }
    const sigilTrait1 = toHashString(sigil.firstTraitId ?? 0);
    const sigilTrait2 = toHashString(sigil.secondTraitId ?? 0);

    const isLucySigil = sigilTrait1 === "dbe1d775" || sigilTrait1 === "8d2adb6e" || sigilTrait1 === "5c862e13";
    if (isLucySigil && sigilTrait2 !== "dc584f60") {
      cheats.push(`Modified sigil: Lucy sigil with invalid second trait`);
    }

    const isWarElemental = sigilTrait1 === "4c588c27";
    if (isWarElemental && sigilTrait2 !== "887ae0b0") {
      cheats.push(`Modified sigil: War Elemental sigil with invalid second trait`);
    }
  }

  return cheats.join("\n");
};
