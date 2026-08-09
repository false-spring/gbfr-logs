import {
  ActionType,
  AuraSource,
  CharacterType,
  ComputedPlayerState,
  EncounterState,
  EnemyState,
  EnemyType,
  MeterColumns,
  PlayerData,
  PlayerState,
  SbaCause,
  SkillState,
  SortDirection,
  SortType,
} from "@/types";

import EnemyMergeGroups from "@/assets/enemy-merge-groups";

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

export const isAttributedCause = (cause: SbaCause): boolean =>
  typeof cause === "object"
    ? Object.hasOwn(cause, "Action") || Object.hasOwn(cause, "Inferred")
    : cause === "DamageTaken" ||
      cause === "InferredDamageTaken" ||
      cause === "ChainGrant" ||
      cause === "InferredChainGrant" ||
      cause === "InferredRedistribute" ||
      cause === "InferredPartyFill" ||
      cause === "InferredSummonCall";

// Actor ids at or above this encode a party slot (`PLAYER_ID_BASE | slot`);
// anything below is pointer-derived. Mirrors `protocol::PLAYER_ID_BASE`.
export const PLAYER_ID_BASE = 0x80000000;

export const partySlotFromActorIndex = (actorIndex: number): number =>
  actorIndex >= PLAYER_ID_BASE ? actorIndex - PLAYER_ID_BASE : -1;

export const resolvePartySlotIndex = (partyData: (PlayerData | null)[], actorIndex: number): number => {
  const joined = partyData.findIndex((partyMember) => partyMember?.actorIndex === actorIndex);
  return joined !== -1 ? joined : partySlotFromActorIndex(actorIndex);
};

export const playerStunPerHit = (player: ComputedPlayerState): number => {
  const stunHits = player.skillBreakdown.reduce((acc, skill) => acc + skill.stunHits, 0);
  return stunHits === 0 ? 0 : player.totalStunValue / stunHits;
};

// Several Conflux auras share action id 99999; `auraSource` keeps them separate rows.
export const skillRowKey = (skill: {
  actionType: ActionType;
  childCharacterType: CharacterType;
  auraSource?: AuraSource;
}): string => {
  const at = skill.actionType;
  const action = typeof at === "object" ? Object.entries(at)[0].join("-") : at;
  const ct = skill.childCharacterType;
  const child = typeof ct === "string" ? ct : `unknown-${ct.Unknown}`;
  const aura = !skill.auraSource || skill.auraSource === "None" ? "" : `-${skill.auraSource}`;
  return `${child}-${action}${aura}`;
};

export const enemyTypeHash = (et: EnemyType): string =>
  typeof et === "object" ? et.Unknown.toString(16).padStart(8, "0") : String(et);

const mergeGroupIdByHash: Record<string, string> = {};
for (const [groupId, group] of Object.entries(EnemyMergeGroups)) {
  for (const hash of group.members) mergeGroupIdByHash[hash] = groupId;
}

export type EnemyGroup = {
  representativeIndex: number;
  representative: EnemyState;
  indices: number[];
  totalDamage: number;
};

export const buildEnemyGroups = (enemies: EnemyState[]): EnemyGroup[] => {
  const groups: EnemyGroup[] = [];
  const groupById: Record<string, EnemyGroup> = {};

  for (const enemy of enemies) {
    const groupId = mergeGroupIdByHash[enemyTypeHash(enemy.targetType)];
    const existing = groupId ? groupById[groupId] : undefined;

    if (existing) {
      existing.indices.push(enemy.index);
      existing.totalDamage += enemy.totalDamage;
      if (enemy.totalDamage > existing.representative.totalDamage) {
        existing.representativeIndex = enemy.index;
        existing.representative = enemy;
      }
    } else {
      const group: EnemyGroup = {
        representativeIndex: enemy.index,
        representative: enemy,
        indices: [enemy.index],
        totalDamage: enemy.totalDamage,
      };
      groups.push(group);
      if (groupId) groupById[groupId] = group;
    }
  }

  return groups.sort((a, b) => b.totalDamage - a.totalDamage);
};

export const minNullable = (a: number | null, b: number | null): number | null =>
  a === null ? b : b === null ? a : Math.min(a, b);
export const maxNullable = (a: number | null, b: number | null): number | null =>
  a === null ? b : b === null ? a : Math.max(a, b);

const mergeSkillBreakdown = (into: SkillState[], from: SkillState[]) => {
  for (const skill of from) {
    const key = skillRowKey(skill);
    const existing = into.find((s) => skillRowKey(s) === key);
    if (!existing) {
      into.push({ ...skill });
    } else {
      existing.hits += skill.hits;
      existing.totalDamage += skill.totalDamage;
      existing.totalStunValue += skill.totalStunValue;
      existing.stunHits += skill.stunHits;
      existing.minDamage = minNullable(existing.minDamage, skill.minDamage);
      existing.maxDamage = maxNullable(existing.maxDamage, skill.maxDamage);
      existing.minStunValue = minNullable(existing.minStunValue, skill.minStunValue);
      existing.maxStunValue = maxNullable(existing.maxStunValue, skill.maxStunValue);
    }
  }
};

export const combineEncounterStates = (states: EncounterState[]): EncounterState => {
  const startTime = Math.min(...states.map((s) => s.startTime));
  const endTime = Math.max(...states.map((s) => s.endTime));
  const durationSeconds = Math.max(endTime - startTime, 1) / 1000;

  const party: Record<string, PlayerState> = {};
  for (const state of states) {
    for (const [key, player] of Object.entries(state.party)) {
      const existing = party[key];
      if (!existing) {
        party[key] = { ...player, skillBreakdown: player.skillBreakdown.map((skill) => ({ ...skill })) };
      } else {
        existing.totalDamage += player.totalDamage;
        existing.totalStunValue += player.totalStunValue;
        existing.sba = Math.max(existing.sba, player.sba);
        existing.lastDamageTime = Math.max(existing.lastDamageTime, player.lastDamageTime);
        mergeSkillBreakdown(existing.skillBreakdown, player.skillBreakdown);
      }
    }
  }

  let totalDamage = 0;
  for (const player of Object.values(party)) {
    player.dps = player.totalDamage / durationSeconds;
    player.stunPerSecond = player.totalStunValue / durationSeconds;
    totalDamage += player.totalDamage;
  }

  const targets: Record<number, EnemyState> = {};
  for (const state of states) Object.assign(targets, state.targets);

  return {
    totalDamage,
    dps: totalDamage / durationSeconds,
    startTime,
    endTime,
    party,
    status: states[0]?.status ?? "Stopped",
    targets,
  };
};

export const sortPlayers = (players: ComputedPlayerState[], sortType: SortType, sortDirection: SortDirection) => {
  players.sort((a, b) => {
    if (sortType === MeterColumns.Name) {
      return sortDirection === "asc" ? a.partyIndex - b.partyIndex : b.partyIndex - a.partyIndex;
    } else if (sortType === MeterColumns.DPS) {
      return sortDirection === "asc" ? a.dps - b.dps : b.dps - a.dps;
    } else if (sortType === MeterColumns.TotalDamage) {
      return sortDirection === "asc" ? a.totalDamage - b.totalDamage : b.totalDamage - a.totalDamage;
    } else if (sortType === MeterColumns.DamagePercentage) {
      return sortDirection === "asc" ? a?.percentage - b?.percentage : b?.percentage - a?.percentage;
    } else if (sortType === MeterColumns.SBA) {
      return sortDirection === "asc" ? a?.sba - b?.sba : b?.sba - a?.sba;
    } else if (sortType === MeterColumns.TotalSbaAdded || sortType === MeterColumns.SbaPercentage) {
      return sortDirection === "asc"
        ? (a?.totalSbaAdded ?? 0) - (b?.totalSbaAdded ?? 0)
        : (b?.totalSbaAdded ?? 0) - (a?.totalSbaAdded ?? 0);
    } else if (sortType === MeterColumns.TotalStunValue) {
      return sortDirection === "asc" ? a?.totalStunValue - b?.totalStunValue : b?.totalStunValue - a?.totalStunValue;
    } else if (sortType === MeterColumns.StunPerSecond) {
      return sortDirection === "asc" ? a?.stunPerSecond - b?.stunPerSecond : b?.stunPerSecond - a?.stunPerSecond;
    } else if (sortType === MeterColumns.StunPerHit) {
      return sortDirection === "asc"
        ? playerStunPerHit(a) - playerStunPerHit(b)
        : playerStunPerHit(b) - playerStunPerHit(a);
    } else if (sortType === MeterColumns.StunPercentage) {
      return sortDirection === "asc" ? a?.totalStunValue - b?.totalStunValue : b?.totalStunValue - a?.totalStunValue;
    } else if (sortType === MeterColumns.HealDone) {
      return sortDirection === "asc"
        ? (a?.healDone ?? 0) - (b?.healDone ?? 0)
        : (b?.healDone ?? 0) - (a?.healDone ?? 0);
    }

    return 0;
  });
};
