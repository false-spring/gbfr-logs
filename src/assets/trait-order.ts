import { readTextFile } from "@tauri-apps/api/fs";
import { resolveResource } from "@tauri-apps/api/path";

/**
 * One trait's ordering metadata, for grouping and sorting the Effective Traits
 * tab. Keys are lowercase-hex trait hashes, the same the hook ships in
 * `EffectiveTrait.hash`.
 */
export type TraitOrderEntry = {
  name: string;
  category: number;
  category_name: string;
  order: number;
  /** 0/1/2 for awakening v0, awakening v1, warpath, etc. */
  variant: number;
  /** "awakening" / "warpath" / "boundary" for character-specific rows, else null. */
  special: string | null;
};

type TraitOrderCatalog = {
  [hash: string]: TraitOrderEntry;
};

const loadTraitOrder = async () => {
  const resourcePath = await resolveResource("assets/trait-order.json");
  const data = JSON.parse(await readTextFile(resourcePath));
  delete data._meta;
  return data;
};

const TraitOrder = (await loadTraitOrder()) as TraitOrderCatalog;

export const TRAIT_CATEGORY_ORDER = [
  { category: 0, key: "basic" },
  { category: 1, key: "attack" },
  { category: 2, key: "defense" },
  { category: 3, key: "support" },
  { category: 4, key: "special" },
];

export default TraitOrder;
