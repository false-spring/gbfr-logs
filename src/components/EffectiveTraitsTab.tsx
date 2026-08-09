import { Group, Table, Text } from "@mantine/core";
import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import TraitOrder, { TRAIT_CATEGORY_ORDER } from "@/assets/trait-order";
import { type PlayerData } from "@/types";
import { categoryTint } from "@/utils/equipment";
import { toHashString } from "@/utils/format";
import { translateTraitId } from "@/utils/i18n";

const traitDisplayName = (hash: number): string => {
  const entry = TraitOrder[toHashString(hash)];
  if (entry) return entry.name;
  const translated = translateTraitId(hash);
  return translated || `#${toHashString(hash)}`;
};

const CATASTROPHE_LABEL = "Catastrophe";
const CATASTROPHE_NAMES = new Set(["Catastrophe", "Catastrophe Nova"]);

// Trait names pinned to the top of their category, in this order; the rest
// keep `order`. Kept in step with the same list in the companion site's js/traits.js.
const CATEGORY_TOP_SORT: Record<number, string[]> = {
  1: [
    "War Elemental",
    "Unbound Master",
    "Unbound Exertion",
    "Unbound Strike",
    "Unbound Technique",
    "Supernova",
    "Catastrophe",
    "Catastrophe Nova",
    "Fatebreaker",
    "Celestial Incendo",
    "Celestial Aqua",
    "Celestial Terra",
    "Celestial Ventus",
    "Celestial Lumen",
    "Celestial Nyx",
    "DMG Cap",
    "Berserker Echo",
    "Spartan Echo",
    "Supplementary DMG",
    "Guard Payback",
    "Dodge Payback",
  ],
  2: ["Improved Dodge", "Firm Stance", "Flight over Fight", "Untouchable"],
  3: ["Uplift", "Quick Cooldown", "Cascade", "Precise Wrath", "Nimble Onslaught"],
  4: ["Alpha", "Beta", "Gamma", "Potion Hoarder", "Autorevive", "Guts", "Stout Heart"],
};

// The game files these as Special; they read better with the damage traits.
const ATTACK_OVERRIDE = new Set([
  "Unbound Master",
  "Unbound Exertion",
  "Unbound Strike",
  "Unbound Technique",
  "Supernova",
]);

const SIGIL_BOOSTER_NAME = "Sigil Booster";

type EffTraitCell = { name: string; level: number } | null;
type EffTraitRow = { key: string; cells: EffTraitCell[] };
type EffTraitSection = { category: number; key: string; rows: EffTraitRow[] };

const effectiveTraitCategory = (hash: number): number => {
  if (ATTACK_OVERRIDE.has(traitDisplayName(hash))) return 1;
  return TraitOrder[toHashString(hash)]?.category ?? 4;
};

const effectiveTraitSections = (players: PlayerData[]): EffTraitSection[] => {
  const allHashes = new Set<number>();
  players.forEach((p) => (p.effectiveTraits ?? []).forEach((et) => allHashes.add(et.hash)));

  const orderOf = (hash: number) => TraitOrder[toHashString(hash)]?.order ?? 999999;
  const specialOf = (hash: number) => TraitOrder[toHashString(hash)]?.special ?? null;
  const isContainerTrait = (hash: number) => specialOf(hash) === "awakening" || specialOf(hash) === "warpath";

  const singleCells = (hash: number): EffTraitCell[] =>
    players.map((p) => {
      const et = (p.effectiveTraits ?? []).find((e) => e.hash === hash);
      return et ? { name: traitDisplayName(hash), level: et.level } : null;
    });

  const catastropheCells = (): EffTraitCell[] =>
    players.map((p) => {
      let best: EffTraitCell = null;
      (p.effectiveTraits ?? []).forEach((e) => {
        if (CATASTROPHE_NAMES.has(traitDisplayName(e.hash)) && (best === null || e.level > best.level)) {
          best = { name: traitDisplayName(e.hash), level: e.level };
        }
      });
      return best;
    });

  const containerCells = (special: string, variant: number | null): EffTraitCell[] =>
    players.map((p) => {
      for (const e of p.effectiveTraits ?? []) {
        const meta = TraitOrder[toHashString(e.hash)];
        if (meta && meta.special === special && (variant === null || meta.variant === variant)) {
          return { name: meta.name, level: e.level };
        }
      }
      return null;
    });

  const prioIndex = (category: number, name: string) => {
    const idx = (CATEGORY_TOP_SORT[category] ?? []).indexOf(name);
    return idx === -1 ? Infinity : idx;
  };

  return TRAIT_CATEGORY_ORDER.map(({ category, key }) => {
    const normalHashes = [...allHashes].filter((h) => {
      if (effectiveTraitCategory(h) !== category) return false;
      if (isContainerTrait(h)) return false;
      if (category === 1 && CATASTROPHE_NAMES.has(traitDisplayName(h))) return false;
      if (category === 4 && traitDisplayName(h) === SIGIL_BOOSTER_NAME) return false;
      return true;
    });

    type Desc = { key: string; name: string; order: number; cells: EffTraitCell[] };
    const descs: Desc[] = normalHashes.map((h) => ({
      key: `h${toHashString(h)}`,
      name: traitDisplayName(h),
      order: orderOf(h),
      cells: singleCells(h),
    }));

    if (category === 1) {
      const cCells = catastropheCells();
      if (cCells.some((c) => c !== null)) {
        const familyOrders = [...allHashes].filter((h) => CATASTROPHE_NAMES.has(traitDisplayName(h))).map(orderOf);
        descs.push({ key: "catastrophe", name: CATASTROPHE_LABEL, order: Math.min(...familyOrders), cells: cCells });
      }
    }

    descs.sort((a, b) => {
      const pa = prioIndex(category, a.name);
      const pb = prioIndex(category, b.name);
      if (pa !== pb) return pa - pb;
      return a.order - b.order;
    });

    const rows: EffTraitRow[] = descs.map((d) => ({ key: d.key, cells: d.cells }));

    if (category === 4) {
      if ([...allHashes].some(isContainerTrait)) {
        rows.push({ key: "awakening-0", cells: containerCells("awakening", 0) });
        rows.push({ key: "awakening-1", cells: containerCells("awakening", 1) });
        rows.push({ key: "warpath", cells: containerCells("warpath", null) });
      }
      const sigilHash = [...allHashes].find((h) => traitDisplayName(h) === SIGIL_BOOSTER_NAME);
      if (sigilHash !== undefined) {
        const cells = singleCells(sigilHash);
        if (cells.some((c) => c !== null)) rows.push({ key: `h${toHashString(sigilHash)}`, cells });
      }
    }

    return { category, key, rows };
  }).filter((section) => section.rows.length > 0);
};

export type EffectiveTraitsTabProps = {
  playerData: PlayerData[];
  playerNames: string[];
};

export const EffectiveTraitsTab = ({ playerData, playerNames }: EffectiveTraitsTabProps) => {
  const { t } = useTranslation();

  return (
    <Group mt="20" gap="xs">
      <Table striped layout="fixed">
        <Table.Thead>
          <Table.Tr>
            {playerData.map((player, i) => (
              <Table.Th key={player.actorIndex}>
                <Text fw={700} size="sm">
                  {playerNames[i]}
                </Text>
              </Table.Th>
            ))}
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {effectiveTraitSections(playerData).map((section) => (
            <Fragment key={section.category}>
              <Table.Tr>
                <Table.Td colSpan={playerData.length} style={{ backgroundColor: categoryTint(section.category) }}>
                  <Text fw={700} size="xs">
                    {t(`ui.trait-category.${section.key}`, section.key)}
                  </Text>
                </Table.Td>
              </Table.Tr>
              {section.rows.map((row) => (
                <Table.Tr key={row.key}>
                  {playerData.map((player, i) => {
                    const cell = row.cells[i];
                    return (
                      <Table.Td key={player.actorIndex}>
                        <Text size="xs" fw={300}>
                          {cell ? `${cell.name} Lv.${cell.level}` : "-"}
                        </Text>
                      </Table.Td>
                    );
                  })}
                </Table.Tr>
              ))}
            </Fragment>
          ))}
        </Table.Tbody>
      </Table>
    </Group>
  );
};
