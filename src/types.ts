export type CharacterType = string | { Unknown: number };

export type PlayerData = {
  index: number;
  // @TODO(false): Handle unknown CharacterTypes
  characterType: CharacterType;
  totalDamage: number;
  dps: number;
  lastDamageTime: number;
};

// Calculated fields
export type ComputedPlayerData = PlayerData & {
  percentage: number;
};

export type EncounterStatus = "Waiting" | "InProgress";

export type EncounterState = {
  totalDamage: number;
  dps: number;
  startTime: number;
  endTime: number;
  party: Record<string, PlayerData>;
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
