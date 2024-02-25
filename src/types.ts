export type CharacterType = string | { Unknown: number };

export type PlayerData = {
  index: number;
  // @TODO(false): Handle unknown CharacterTypes
  character_type: CharacterType;
  total_damage: number;
  dps: number;
  last_damage_time: number;

  // Calculated fields
  percentage: number;
};

export type EncounterState = {
  total_damage: number;
  dps: number;
  start_time: number;
  end_time: number;
  party: Record<string, PlayerData>;
};

export type EncounterUpdateEvent = {
  event: string;
  payload: EncounterState;
};

export type EncounterResetEvent = {
  event: string;
  payload: EncounterState;
};
