import { readTextFile } from "@tauri-apps/api/fs";
import { resolveResource } from "@tauri-apps/api/path";

/**
 * One skillboard node from the bundled master-traits catalog. Keys are
 * lowercase SBE_* effect hashes, the same the hook ships in
 * `PlayerData.masterTraitFlags`.
 */
export type MasterTraitNode = {
  /** SB_DEF is "Insight", SB_ATK "Essence", SB_LIMIT "Crux". */
  board: "SB_DEF" | "SB_ATK" | "SB_LIMIT";
  group: "rank1" | "rank2" | "rank3" | "EX";
  /** 50 = gem node, 100 = style/gate row. */
  cost: number;
  card: boolean;
};

type MasterTraitCatalog = {
  [hash: string]: MasterTraitNode;
};

const loadMasterTraits = async () => {
  const resourcePath = await resolveResource("assets/master-traits.json");
  const data = JSON.parse(await readTextFile(resourcePath));
  delete data._meta;
  return data;
};

const MasterTraits = (await loadMasterTraits()) as MasterTraitCatalog;

export default MasterTraits;
