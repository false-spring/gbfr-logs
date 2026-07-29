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
 * ActionType represents the type of action that a skill can be. Names must
 * match the Rust `protocol::ActionType` variants; they arrive as serde-tagged
 * keys. `Group` is the exception — no Rust counterpart, never on the wire,
 * synthesized client-side by `useSkillBreakdown` for a folded skill group.
 *
 * Examples:
 * - `"LinkAttack"` - Link Attack
 * - `"SBA"` - Skybound Art
 * - `{ SupplementaryDamage: 113 }` (as its key, object with a number representing the skill number)
 * - `{ Normal: 113 }` (as its key, object with a number representing the skill number)
 */
export type ActionType =
  | "LinkAttack"
  | "SBA"
  | { SupplementaryDamage: number }
  | { DamageOverTime: number }
  | { Normal: number }
  | { Group: string };

/**
 * Which Conflux aura a `99999` hit came from. Several auras share that one
 * action id, so it cannot name them; the parser derives this from the damage
 * flags, and — for the two that share a flag word as well — from the proc the
 * hit trails. "None" for every hit that isn't one of them.
 */
export type AuraSource =
  | "None"
  | "ToxicBlast"
  | "LusterOfDarkness"
  | "ReflectionOfAFallenFort"
  | "MysteryBox"
  | "IceAndFireFollowUp";

export type SkillState = {
  /** ActionType of the skill */
  actionType: ActionType;
  /** For some characters, the skill can be a child of another character type. */
  childCharacterType: CharacterType;
  /**
   * Which Conflux aura produced this, when the action id alone can't say.
   * Optional: absent on rows from logs saved before this existed, and on
   * condensed skill groups, which have no single source.
   */
  auraSource?: AuraSource;
  /** Number of total hits of the skill */
  hits: number;
  /** Minimum damage of the skill */
  minDamage: number | null;
  /** Maximum damage of the skill */
  maxDamage: number | null;
  /** Total damage of the skill */
  totalDamage: number;
  /** Total stun value of the skill hits */
  totalStunValue: number;
  /** Minimum recorded stun value of the skill */
  minStunValue: number | null;
  /** Maximum recorded stun value of the skill */
  maxStunValue: number | null;
  /** Number of hits that dealt non-zero stun (denominator for stun-per-hit) */
  stunHits: number;
};

export type ComputedSkillState = SkillState & {
  /** Damage contribution as a percentage of the total */
  percentage: number;
};

export type ComputedSkillGroup = ComputedSkillState & {
  /** Skills */
  skills?: ComputedSkillState[];
};

/**
 * Why a player's SBA gauge moved (parser `SbaCause`).
 *
 * `Action` is read from the causing call. `Inferred` is a temporal join — a
 * replicated peer counter paired with the gauge sync that followed it — and is
 * deliberately a separate variant so the UI can show provenance rather than
 * quietly presenting a guess as a reading. `Remote` is a network gauge sync,
 * which carries no action information at any layer.
 */
export type SbaCause =
  | "NotClassified"
  | "Remote"
  | "Unknown"
  | "HookUnavailable"
  | "DamageTaken"
  | "InferredDamageTaken"
  | "ChainGrant"
  | "InferredChainGrant"
  | "InferredRedistribute"
  | "InferredPartyFill"
  | "InferredSummonCall"
  | { Action: ActionType }
  | { Inferred: ActionType };

/**
 * One per-cause SBA gauge-generation row.
 */
export type SbaSourceState = {
  cause: SbaCause;
  /**
   * Who performed the move, when that is someone other than the player — Id's
   * dragon, Ferry's pet, Seofon's avatar. `null` for everyone else, and for
   * causes that name no move at all.
   *
   * Gauge messages carry no actor, so the parser recovers this by correlating
   * with the damage event under the same action id. It matters because action
   * ids are only unique per actor: Id's `200` is "Lunge 1" in human form and
   * "Light Blast" as a dragon.
   */
  childCharacterType: CharacterType | null;
  /** Gauge-update events folded into this row — NOT a hit count. */
  ticks: number;
  /** Gauge generated, 0..1000 scale (1000 = one full bar). */
  totalSbaAdded: number;
};

export type ComputedSbaSourceState = SbaSourceState & {
  /** Share of the player's total generated gauge. */
  percentage: number;
};

/**
 * Several gauge rows condensed into one, the same way `ComputedSkillGroup`
 * condenses a combo on the damage and stun tabs — and using the same
 * `skill-groups.json` mapping, so a move belongs to the same group whichever
 * tab it is read on.
 *
 * `cause` is the synthetic `{ Action: { Group: <name> } }`, which the existing
 * name resolution already understands.
 */
export type ComputedSbaGroup = ComputedSbaSourceState & {
  /** The rows folded in, for the expanded view. */
  sources: ComputedSbaSourceState[];
};

/** Healing split by source category (potion and drain share `selfRecovery`;
 * they can't be told apart at the hook). Mirrors the Rust `HealBreakdown`. */
export type HealBreakdown = {
  skill: number;
  selfRecovery: number;
  regen: number;
  revive: number;
  other: number;
};

/** The heal categories in display order, with their i18n key suffix. */
export const HEAL_CATEGORIES: { key: keyof HealBreakdown; label: string }[] = [
  { key: "skill", label: "heal-cat-skill" },
  { key: "selfRecovery", label: "heal-cat-self-recovery" },
  { key: "regen", label: "heal-cat-regen" },
  { key: "revive", label: "heal-cat-revive" },
  { key: "other", label: "heal-cat-other" },
];

export type PlayerState = {
  /** Unique ID for this player */
  index: number;
  /** Character type of this player. (Pl1000 / Pl1800 / ..) */
  characterType: CharacterType;
  /** Total damage dealt */
  totalDamage: number;
  /** DPS over the encounter time */
  dps: number;
  /** Amount of SBA Gauge (0.0 - 1000.0) */
  sba: number;
  /** Total stun value */
  totalStunValue: number;
  /** Stun per second over the encounter time */
  stunPerSecond: number;
  /** Time of the last damage dealt */
  lastDamageTime: number;
  /** Stats for individual skills logged */
  skillBreakdown: SkillState[];
  /** Damage this player TOOK. Never mixed into totalDamage (damage dealt). */
  totalDamageTaken: number;
  /**
   * Effective HP this player RESTORED — healing done, credited to the healer.
   * Counts HP actually put back, not overheal. Never mixed into any damage
   * counter.
   */
  healDone: number;
  /** Effective HP this player RECEIVED — healing taken, credited to the healed.
   * The mirror of healDone. */
  healReceived: number;
  /** Healing provided, split by source category. Sums to healDone. */
  healProvidedByType: HealBreakdown;
  /** Healing received, split by source category. Sums to healReceived. */
  healReceivedByType: HealBreakdown;
  /** Per-cause SBA gauge generation over the encounter */
  sbaBreakdown: SbaSourceState[];
  /**
   * Total gauge generated (sum of sbaBreakdown, including the rows that carry
   * no action). Distinct from `sba`, which is the CURRENT gauge level and
   * resets to zero every time the player spends it.
   */
  totalSbaAdded: number;
};

export type ComputedPlayerState = PlayerState & {
  /** Damage contribution as a percentage of the total */
  percentage: number;
  /** Actual party index */
  partyIndex: number;
};

export type EnemyState = {
  /** Enemy index */
  index: number;
  /** Enemy type (resolved variant class) */
  targetType: EnemyType;
  /** Base class type — naming fallback when `targetType` isn't in the name table */
  baseTargetType: EnemyType;
  /** Total damage done to this target */
  totalDamage: number;
};

export type EncounterStatus = "Waiting" | "InProgress" | "Stopped";

/**
 * Hand-mirrored from the Rust `DerivedEncounterState` (`src-tauri/src/parser/v1/mod.rs`,
 * `#[serde(rename_all = "camelCase")]`), which also serializes `totalStunValue` and
 * `stunPerSecond`. Those two are deliberately absent here: `combineEncounterStates`
 * (`utils/derive.ts`) builds merged `EncounterState`s for enemy merge-groups entirely on the
 * frontend, so an encounter-level stun field on this type would be a frontend invention
 * on those states rather than a backend reading. Encounter-level stun is re-derived from
 * the party rows instead (`Table.tsx`).
 */
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
  party: Record<string, PlayerState>;
  /** Status of the encounter */
  status: EncounterStatus;
  /** Targets for this encounter */
  targets: Record<number, EnemyState>;
};

/**
 * What the live `encounter-update` / `on-area-enter` events actually carry
 * (Rust `LiveEncounterUpdate`, `parser/v1/encounter.rs`): `EncounterState`
 * minus `targets`, which grows for the life of an encounter and is only
 * rendered by the log view, where it arrives via `fetch_encounter_state`
 * instead. A full `EncounterState` is assignable wherever this type is
 * expected.
 */
export type LiveEncounterState = Omit<EncounterState, "targets">;

export type EncounterUpdateEvent = {
  event: string;
  payload: LiveEncounterState;
};

export type EncounterResetEvent = {
  event: string;
  payload: EncounterState;
};

export type Sigil = {
  firstTraitId: number;
  firstTraitLevel: number;
  secondTraitId: number;
  secondTraitLevel: number;
  sigilId: number;
  equippedCharacter: number;
  sigilLevel: number;
  acquisitionCount: number;
  notificationEnum: number;
};

export type WeaponInfo = {
  weaponId: number;
  starLevel: number;
  plusMarks: number;
  awakeningLevel: number;
  trait1Id: number;
  trait1Level: number;
  trait2Id: number;
  trait2Level: number;
  trait3Id: number;
  trait3Level: number;
  /** Endless Ragnarok: weapons carry up to 5 trait pairs. */
  trait4Id: number;
  trait4Level: number;
  trait5Id: number;
  trait5Level: number;
  wrightstoneId: number;
  weaponLevel: number;
  /** Endless Ragnarok: transcendence ("rebuild") level; 0 = none. */
  transcendenceLevel: number;
  /** Endless Ragnarok: awakening level; 0 = none. */
  awakeningLevelEr: number;
  weaponHp: number;
  weaponAttack: number;
  /** Endless Ragnarok: wrightstone trait pairs captured record-side. */
  wrightstoneTrait1Id: number;
  wrightstoneTrait1Level: number;
  wrightstoneTrait2Id: number;
  wrightstoneTrait2Level: number;
  wrightstoneTrait3Id: number;
  wrightstoneTrait3Level: number;
};

/** Pre-ER logs only; Endless Ragnarok logs carry equipped summons instead. */
export type Overmastery = {
  id: number;
  flags: number;
  value: number;
};

export type OvermasteryInfo = {
  overmasteries: Overmastery[];
};

/** One equipped summon (Endless Ragnarok): grants an aura trait + an equip bonus. */
export type SummonSlot = {
  id: number;
  /** Aura trait id, shown in-game as trait name + "T.Lvl". */
  traitId: number;
  /** Aura trait level ("T.Lvl"; -1 when uninitialized). */
  traitLevel: number;
  /** Equip-bonus id hash (0x887AE0B0 = none). */
  equipBonusId: number;
  /** Equip-bonus level: 0-based index into the bonus's value ladder (-1 = uninit). */
  equipBonusLevel: number;
};

export type SummonInfo = {
  summons: SummonSlot[];
};

/** One active Over Mastery bonus (Endless Ragnarok) — the real ER over mastery. */
export type OverMasteryLine = {
  id: number;
  /** Star rank 1..10; 0 = empty slot. */
  rank: number;
  /** Raw per-rank value; display = value x per-param multiplier (see overMasteryDisplayValue). */
  value: number;
};

/** One skillboard flag row (Endless Ragnarok): SBE_* effect hash + allocated flag. */
export type MasterTraitFlag = {
  hash: number;
  on: boolean;
};

/** One consolidated post-cap effective trait (Endless Ragnarok): trait hash + 0..99 level. */
export type EffectiveTrait = {
  hash: number;
  level: number;
};

export type PlayerStats = {
  level: number;
  totalHp: number;
  totalAttack: number;
  stunPower: number;
  criticalRate: number;
  totalPower: number;
  /** Raw DMG-cap accumulator channels; captured but not displayed. */
  dmgCapChannels: [number, number, number];
};

export type PlayerData = {
  actorIndex: number;
  displayName: string;
  characterName: string;
  /** Online-identity metadata (rides in saved logs; never displayed). */
  networkUserId: string | null;
  /** Online-identity metadata (rides in saved logs; never displayed). */
  networkUserName: string | null;
  characterType: CharacterType;
  sigils: Sigil[];
  isOnline: boolean;
  weaponInfo: WeaponInfo | null;
  /** Pre-ER logs only. */
  overmasteryInfo: OvermasteryInfo | null;
  /** Endless Ragnarok logs only. */
  summonInfo: SummonInfo | null;
  /** Endless Ragnarok logs only: the four equipped skill ability-ids (0 = empty). */
  skillLoadout?: number[];
  /** Endless Ragnarok logs only: the four active Over Mastery bonuses. */
  overMastery?: OverMasteryLine[];
  /** Endless Ragnarok logs only: the skillboard flag band. */
  masterTraitFlags?: MasterTraitFlag[];
  /** Endless Ragnarok logs only: consolidated post-cap effective traits. */
  effectiveTraits?: EffectiveTrait[];
  /** Endless Ragnarok logs only: master level 0..=55.
   *
   * ONE stored number that the game draws as two — `min(v, 50)` is the
   * displayed Master Lvl and `max(0, v - 50)` is the Mastery Break star count
   * (see `splitMasterLevel`). Level-synced DOWN in a capped quest, so a
   * character with breaks can legitimately read 50 in one, and this is what
   * the record said during that encounter rather than the account ceiling. */
  masterLevel?: number | null;
  playerStats: PlayerStats | null;
};

/** Where the displayed Master Lvl stops and the Mastery Break stars begin.
 *
 *  Nothing reads this or `splitMasterLevel` yet — both are kept deliberately,
 *  for a planned master-level display that draws the stored number the way the
 *  game does. */
export const MASTER_LEVEL_DISPLAY_CAP = 50;

/** The two numbers the game draws from one stored master level. Mirrors
 *  `protocol::split_master_level` — keep the two in step. Currently unused;
 *  see the note on `MASTER_LEVEL_DISPLAY_CAP` above. */
export const splitMasterLevel = (masterLevel: number): { level: number; breaks: number } => ({
  level: Math.min(masterLevel, MASTER_LEVEL_DISPLAY_CAP),
  breaks: Math.max(0, masterLevel - MASTER_LEVEL_DISPLAY_CAP),
});

export type PartyUpdateEvent = {
  event: string;
  payload: Array<PlayerData | null>;
};

export enum MeterColumns {
  Name = "name",
  DPS = "dps",
  TotalDamage = "damage",
  DamagePercentage = "damage-percentage",
  SBA = "sba",
  TotalStunValue = "total-stun-value",
  StunPerSecond = "stun-per-second",
  StunPerHit = "stun-per-hit",
  StunPercentage = "stun-percentage",
  TotalSbaAdded = "total-sba-added",
  SbaPercentage = "sba-percentage",
  HealDone = "heal-done",
}

export enum SkillMeterColumns {
  Hits = "hits",
  TotalDamage = "total-damage",
  MinDamage = "min-damage",
  MaxDamage = "max-damage",
  AverageDamage = "average-damage",
  DamagePercentage = "skill-damage-percentage",
  TotalStunValue = "skill-total-stun-value",
  MinStunValue = "min-stun-value",
  MaxStunValue = "max-stun-value",
  StunPerHit = "skill-stun-per-hit",
  StunPerSecond = "skill-stun-per-second",
  StunPercentage = "skill-stun-percentage",
  /**
   * Gauge-update events folded into a row. Deliberately NOT called "hits" —
   * one call to the game's SBA-update function may cover several hits or none,
   * and the counters produce gauge with no hit at all.
   */
  SbaTicks = "sba-ticks",
  SbaTotalAdded = "sba-total-added",
  SbaPerTick = "sba-per-tick",
  SbaPercentage = "skill-sba-percentage",
}

export type SortType = MeterColumns;

export type LogSortType = "time" | "duration" | "quest-elapsed-time";
export type SortDirection = "asc" | "desc";

export type Log = {
  id: number;
  name: string;
  time: number;
  duration: number;
  version: number;
  primaryTarget: EnemyType | null;
  p1Name: string | null;
  p1Type: string | null;
  p2Name: string | null;
  p2Type: string | null;
  p3Name: string | null;
  p3Type: string | null;
  p4Name: string | null;
  p4Type: string | null;
  p1NetworkId: string | null;
  p2NetworkId: string | null;
  p3NetworkId: string | null;
  p4NetworkId: string | null;
  questId: number | null;
  questElapsedTime: number | null;
  questCompleted: boolean;
  gameVersion: string | null;
};

export type SBAEvent = [
  number,
  (
    | { OnAttemptSBA: { actor_index: number } }
    | { OnPerformSBA: { actor_index: number } }
    | { OnContinueSBAChain: { actor_index: number } }
  ),
];

/** One Link Time window as `[startMs, endMs]` from the encounter start. An
 * empty list of these means the windows were never captured (an older log, or
 * a run where the link hooks did not resolve), not that none occurred. */
export type LinkTimeWindow = [number, number];

/** Status-effect uptime: actor grouping id -> status kind id -> the windows that
 * status was up for, `[startMs, endMs]` from the encounter start. Empty for logs
 * recorded before the status hooks existed, same as the Link Time windows. */
export type StatusIntervals = Record<number, Record<number, LinkTimeWindow[]>>;

/** Deepest stack of each status kind held by each actor: actor grouping id ->
 * status kind id -> peak depth. `1` for anything that never stacked, so a label
 * only annotates a status when this exceeds 1. A missing entry means the depth
 * was never captured, which renders the same as never stacking. */
export type StatusPeakStacks = Record<number, Record<number, number>>;

/** One depth observation: `[msFromEncounterStart, depth]`. A STEP sample — the
 * depth held from that moment until the next point, not a value to interpolate
 * between. The trailing `0` is the removal. */
export type StatusStackSample = [number, number];

/** Stack depth over time: actor grouping id -> status kind id -> samples.
 *
 * Only present for statuses whose depth actually rose above 1, so the presence
 * of an entry is itself the signal that a depth chart is worth drawing — a buff
 * that sat at one stack would only ever produce a flat line. */
export type StatusStackSeries = Record<number, Record<number, StatusStackSample[]>>;

/** One magnitude observation: `[msFromEncounterStart, summedValue]`. A STEP
 * sample, like the stack one. Units are fractional — 0.30 is 30%. */
export type StatusValueSample = [number, number];

/** Summed magnitude over time: actor grouping id -> status kind id -> samples.
 *
 * The total is the engine's own rule — stat contributors fold additively, so two
 * DEF DOWN objects at 0.10 and 0.20 act as 0.30. It is the RAW sum: the game also
 * clamps the combined multiplier at use time, but that ceiling is attacker-side,
 * so applying it to a target's chart would show the wrong entity's number.
 *
 * Only present for kinds that carried a non-zero magnitude, so an absent entry
 * is the signal not to draw a chart. Most kinds have none: 90 of the game's 166
 * status classes carry no numeric value at all. */
export type StatusValueSeries = Record<number, Record<number, StatusValueSample[]>>;

/** Status kind id -> whether its magnitude is a FRACTION of a stat rather than a
 * count. A property of the kind, not of the actor holding it.
 *
 * The two cannot be told apart by the number: a DEF DOWN of 0.20 is 20%, while a
 * Burn of 4499 is damage per tick and a Shield of 89901 is hit points. */
export type StatusValueIsFraction = Record<number, boolean>;

/** One source's own uptime for a status, and how the game identified it. */
export type StatusSourceWindows = {
  /** The actor that applied it, in the same id space as the target it landed on.
   * `null` when the source could not be resolved — ordinary for environmental
   * and self-applied statuses, and shown as "Unknown" rather than dropped. */
  applierIndex: number | null;
  /** The game's own source-identity tuple for this application.
   *
   * OPAQUE. It is what the engine's refresh-vs-insert test compares, so it does
   * separate one source's application from another's, but it is NOT known to be
   * an ability id in any namespace this app can name — a live comparison against
   * damage-event action ids found only a partial overlap. Used to split entries,
   * never rendered as an ability. */
  sourceIds: number[] | null;
  windows: LinkTimeWindow[];
  /** This source's OWN magnitude over time, summed across its objects of the
   * kind. Empty for a kind with no numeric magnitude, and for logs recorded
   * before per-object values existed. */
  values: StatusValueSample[];
};

/** Per-SOURCE status uptime: actor grouping id -> status kind id -> one entry
 * per distinct source that applied it.
 *
 * A breakdown of `StatusIntervals`, not a partition of it: several party members
 * debuffing one boss produce OVERLAPPING windows, so these sum to more than the
 * aggregate. Empty for logs recorded before the source was captured, which is
 * the signal to hide the selector entirely. */
export type StatusSources = Record<number, Record<number, StatusSourceWindows[]>>;
