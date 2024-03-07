/**
 * CharacterType represents the type of character that a player can be.
 *
 * Examples:
 * - `"Pl1000"`
 * - `"Pl1800"`
 * - `{ Unknown: 0xF546E414 }`
 */
export type CharacterType = string | { Unknown: number };

/**
 * EnemyType represents the type of enemy.
 *
 * Examples:
 * - `"Em1000"`
 * - `"Em1200"`
 * - `{ Unknown: 0xF546E414 }`
 */
export type EnemyType = string | { Unknown: number };

/**
 * ActionType represents the type of action that a skill can be.
 *
 * Examples:
 * - `"LinkAttack"` - Link Attack
 * - `"SBA"` - Skybound Art
 * - `{ SupplementaryAttack: 113 }` (as its key, object with a number representing the skill number)
 * - `{ Normal: 113 }` (as its key, object with a number representing the skill number)
 */
export type ActionType =
  | "LinkAttack"
  | "SBA"
  | { SupplementaryAttack: number }
  | { DamageOverTime: number }
  | { Normal: number };

export type SkillState = {
  /** ActionType of the skill */
  actionType: ActionType;
  /** For some characters, the skill can be a child of another character type. */
  childCharacterType: CharacterType;
  /** Number of total hits of the skill */
  hits: number;
  /** Minimum damage of the skill */
  minDamage: number | null;
  /** Maximum damage of the skill */
  maxDamage: number | null;
  /** Total damage of the skill */
  totalDamage: number;
};

export type ComputedSkillState = SkillState & {
  /** Damage contribution as a percentage of the total */
  percentage: number;
};

export type PlayerData = {
  /** Unique ID for this player */
  index: number;
  /** Character type of this player. (Pl1000 / Pl1800 / ..) */
  characterType: CharacterType;
  /** Total damage dealt */
  totalDamage: number;
  /** DPS over the encounter time */
  dps: number;
  /** Time of the last damage dealt */
  lastDamageTime: number;
  /** Stats for individual skills logged */
  skills: SkillState[];
};

export type ComputedPlayerData = PlayerData & {
  /** Damage contribution as a percentage of the total */
  percentage: number;
  /** Actual party index */
  partyIndex: number;
};

export type EncounterStatus = "Waiting" | "InProgress" | "Stopped";

export type EncounterState = {
  /** Total damage dealt in the whole encounter */
  totalDamage: number;
  /** Total DPS dealt over the encounter time */
  dps: number;
  /** The time of the encounter's first damage instance (UTC milliseconds since epoch) */
  startTime: number;
  /** The time of the encounter's last known damage instance (UTC milliseconds since epoch) */
  endTime: number;
  /** Represents the players in the encounter */
  party: Record<string, PlayerData>;
  /** Status of the encounter */
  status: EncounterStatus;
};

export type EncounterUpdateEvent = {
  event: string;
  payload: EncounterState;
};

export type EncounterResetEvent = {
  event: string;
  payload: EncounterState;
};

export type SortType = "partyIndex" | "dps" | "damage" | "percentage";
export type SortDirection = "asc" | "desc";
