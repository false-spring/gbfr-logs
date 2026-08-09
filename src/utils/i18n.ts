import { t } from "i18next";

import {
  ActionType,
  AuraSource,
  CharacterType,
  ComputedPlayerState,
  EnemyType,
  PlayerData,
  SbaCause,
  SkillState,
} from "@/types";
import { EMPTY_ID } from "@/utils/constants";
import { toHashString } from "@/utils/format";

const SUMMON_ATTACK_ACTION_ID = 80000;

const CONFLUX_AURA_SENTINEL = 99999;

const AURA_SOURCE_KEYS: Record<Exclude<AuraSource, "None">, string> = {
  ToxicBlast: "conflux-toxic-blast",
  LusterOfDarkness: "conflux-luster-of-darkness",
  ReflectionOfAFallenFort: "conflux-reflection-of-a-fallen-fort",
  MysteryBox: "conflux-mystery-box",
  IceAndFireFollowUp: "conflux-ice-and-fire-follow-up",
};

export const getSbaCauseName = (
  characterType: CharacterType,
  cause: SbaCause,
  childCharacterType: CharacterType | null = null
): string => {
  if (typeof cause === "string") {
    return t(`ui.sba-breakdown.${cause}`);
  }

  const asSkill = (actionType: ActionType) =>
    getSkillName(characterType, {
      actionType,
      childCharacterType: childCharacterType ?? characterType,
    } as SkillState);

  return Object.hasOwn(cause, "Action")
    ? asSkill((cause as { Action: ActionType }).Action)
    : asSkill((cause as { Inferred: ActionType }).Inferred);
};

export const getSkillName = (characterType: CharacterType, skill: SkillState) => {
  switch (true) {
    case skill.actionType === "LinkAttack":
      return t([`skills.${characterType}.link-attack`, "skills.default.link-attack"]);
    case skill.actionType === "SBA":
      return t("skills.default.skybound-arts");
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

      if (skillID === SUMMON_ATTACK_ACTION_ID && typeof skill.childCharacterType === "object") {
        const hash = skill.childCharacterType.Unknown.toString(16).padStart(8, "0");

        return t([`summons:${hash}.text`, `skills.default.${skillID}`, "skills.default.unknown-skill"], {
          id: skillID,
        });
      }

      if (skillID === CONFLUX_AURA_SENTINEL && skill.auraSource && skill.auraSource !== "None") {
        return t([`skills.default.${AURA_SOURCE_KEYS[skill.auraSource]}`, "skills.default.unknown-skill"], {
          id: skillID,
        });
      }

      if (skill.etherGun) {
        return t([`skills.default.${skillID}`, `skills.default.unknown-skill`], { id: skillID });
      }

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
    case typeof skill.actionType == "object" && Object.hasOwn(skill.actionType, "Group"): {
      const actionType = skill.actionType as { Group: string };

      return t(
        [
          `skills.${characterType}.skill-groups.${actionType.Group}`,
          `skills.default.skill-groups.${actionType.Group}`,
          `skills.default.unknown-skill`,
        ],
        { id: actionType.Group }
      );
    }
    default:
      return t("ui.unknown");
  }
};

/// Formats the player name and translates the player's character type.
export const translatedPlayerName = (
  partySlotIndex: number,
  partySlotData: PlayerData | null,
  player?: ComputedPlayerState,
  show_display_names: boolean = true
) => {
  if (!player) return "Guest";

  const ct = player.characterType;
  const characterType =
    typeof ct === "string"
      ? t(`characters:${ct}`, `ui:characters.${ct}`)
      : `#${ct.Unknown.toString(16).padStart(8, "0").toUpperCase()}`;
  const displayName = `${partySlotData?.displayName} (${characterType})`;
  const name = show_display_names && partySlotData?.displayName ? displayName : characterType;

  return `[${partySlotIndex >= 0 ? partySlotIndex + 1 : "Guest"}]` + " " + name;
};

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

// Content ids in the `0x8xxxxx` family are Conflux (endless-mode) areas.
const CONTENT_FAMILY_MASK = 0xf00000;
const CONFLUX_CONTENT_FAMILY = 0x800000;

/// Translates the quest ID to a human-readable string.
export const translateQuestId = (id: number | null): string => {
  if (id === null) return "";
  if ((id & CONTENT_FAMILY_MASK) === CONFLUX_CONTENT_FAMILY) {
    return t("quest.conflux", "The Conflux");
  }
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

export const translateSummonId = (id: number | null): string => {
  if (id === null) return "";
  if (id === EMPTY_ID) return "";

  const hash = id.toString(16).padStart(8, "0");
  return t(`summons:${hash}.text`, "");
};

export const translateSummonBonusId = (id: number | null): string => {
  if (id === null || id === EMPTY_ID) return "";
  const hash = id.toString(16).padStart(8, "0");
  return t(`summonbonuses:${hash}.text`, "");
};
