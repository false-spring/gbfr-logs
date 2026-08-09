import { readTextFile } from "@tauri-apps/api/fs";
import { resolveResource } from "@tauri-apps/api/path";

/**
 * Value ladder for one summon equip bonus. Keys are lowercase-hex equip-bonus
 * id hashes, the same the hook ships in `SummonSlot.equipBonusId`.
 * Display value = ladder[level] * display_mult.
 */
export type SummonEquipBonus = {
  display_mult: number;
  ladder: number[];
};

type SummonEquipBonusCatalog = {
  [hash: string]: SummonEquipBonus;
};

const loadSummonEquipBonuses = async () => {
  const resourcePath = await resolveResource("assets/summon-equip-bonus.json");
  const data = JSON.parse(await readTextFile(resourcePath));
  delete data._meta;
  return data;
};

const SummonEquipBonuses = (await loadSummonEquipBonuses()) as SummonEquipBonusCatalog;

export default SummonEquipBonuses;
