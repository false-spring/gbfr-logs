// Enemy-type variants that spawn as separate actor instances but collapse into
// one enemy-selector entry (see `buildEnemyGroups` in utils/derive). Members
// are enemy-type hashes, lowercase 8-hex, as the `enemies` name table keys them.
export type EnemyMergeGroup = {
  members: string[];
};

const EnemyMergeGroups: Record<string, EnemyMergeGroup> = {
  furycane_alterphase_twins: {
    members: ["67cca534", "fbc2c2a3"],
  },
  the_world: {
    members: ["18fde573", "5a37f37f", "d7ba6d4a"],
  },
};

export default EnemyMergeGroups;
