import { t } from "i18next";

import MasterTraits from "@/assets/master-traits";
import { resolveTextIcons } from "@/models/textIcons";
import { type MasterTraitFlag } from "@/types";
import { toHashString } from "@/utils/format";

const BOARD_ORDER = ["SB_DEF", "SB_ATK", "SB_LIMIT"];
const GEM_GROUP_ORDER = ["rank1", "rank2", "rank3", "EX"];

const BOARD_STYLE_NAMES: Record<string, string> = {
  SB_DEF: "Insight",
  SB_ATK: "Essence",
  SB_LIMIT: "Crux",
};

export type StyleBoardSummary = {
  board: string;
  cardHash: number | null;
  ranksOn: number;
  ranksTotal: number;
  gems: number[];
};

export const styleBoardSummaries = (flags: MasterTraitFlag[]): StyleBoardSummary[] => {
  return BOARD_ORDER.map((board) => {
    const boardFlags = flags.filter((flag) => MasterTraits[toHashString(flag.hash)]?.board === board);
    const rankRows = boardFlags.filter((flag) => MasterTraits[toHashString(flag.hash)].cost === 100);

    return {
      board,
      cardHash: boardFlags.find((flag) => MasterTraits[toHashString(flag.hash)].card)?.hash ?? null,
      ranksOn: rankRows.filter((flag) => flag.on).length,
      ranksTotal: rankRows.length,
      gems: boardGemCounts(flags, board),
    };
  });
};

// Geometric Shapes, not the card suits: U+2666 has an emoji presentation.
export const styleRankDiamonds = (ranksOn: number, ranksTotal: number): string => {
  return "◆".repeat(ranksOn) + "◇".repeat(Math.max(0, ranksTotal - ranksOn));
};

export const styleBoardName = (cardHash: number | null, board: string): string => {
  if (cardHash === null) return BOARD_STYLE_NAMES[board] ?? board;
  return t([`mastertraits:${toHashString(cardHash)}.text`, "ui.unknown"], { id: toHashString(cardHash) });
};

const BOARD_SOURCE_IDS: Record<number, string> = {
  9999: "SB_DEF",
  9998: "SB_ATK",
  9997: "SB_LIMIT",
};

/**
 * The Mastery Trait that granted a status effect.
 */
export const styleBoardSourceName = (sourceId: number, flags: MasterTraitFlag[] | undefined): string | null => {
  const board = BOARD_SOURCE_IDS[sourceId];
  if (board === undefined) return null;

  const cardHash = (flags ?? []).find((flag) => {
    const node = MasterTraits[toHashString(flag.hash)];
    return node?.card === true && node.board === board;
  })?.hash;
  return styleBoardName(cardHash ?? null, board);
};

export const boardGemCounts = (flags: MasterTraitFlag[], board: string): number[] => {
  return GEM_GROUP_ORDER.map(
    (group) =>
      flags.filter((flag) => {
        if (!flag.on) return false;
        const node = MasterTraits[toHashString(flag.hash)];
        return node !== undefined && node.cost === 50 && node.board === board && node.group === group;
      }).length
  );
};

export const masteryLevel = (flags: MasterTraitFlag[]): number => {
  const n = flags.filter((flag) => {
    if (!flag.on) return false;
    const node = MasterTraits[toHashString(flag.hash)];
    return node !== undefined && node.cost === 50;
  }).length;
  return Math.min(50, Math.max(0, n));
};

type MasterTraitNodeDetail = { hash: number; on: boolean; text: string };
type MasterTraitRankGroup = { group: string; nodes: MasterTraitNodeDetail[] };
type MasterTraitStyleBlock = {
  board: string;
  cardHash: number | null;
  perks: MasterTraitNodeDetail[];
  groups: MasterTraitRankGroup[];
};

// The game hard-wraps descriptions for its fixed-width text box.
const collapseTraitWrapArtifacts = (text: string): string => text.replace(/(?<![.:!?])\n/g, " ");

export const masterTraitStyleBlocks = (flags: MasterTraitFlag[]): MasterTraitStyleBlock[] => {
  const known = flags.filter((flag) => MasterTraits[toHashString(flag.hash)] !== undefined);
  const nodeText = (hash: number) =>
    resolveTextIcons(
      collapseTraitWrapArtifacts(t(`mastertraitdetails:${toHashString(hash)}.text`, { defaultValue: "" }))
    );
  const toDetail = (flag: MasterTraitFlag): MasterTraitNodeDetail => ({
    hash: flag.hash,
    on: flag.on,
    text: nodeText(flag.hash),
  });

  return BOARD_ORDER.map((board) => {
    const boardFlags = known.filter((flag) => MasterTraits[toHashString(flag.hash)].board === board);
    const cardHash = boardFlags.find((flag) => MasterTraits[toHashString(flag.hash)].card)?.hash ?? null;

    const perks = boardFlags
      .filter((flag) => MasterTraits[toHashString(flag.hash)].cost === 100)
      .map(toDetail)
      .filter((node) => node.text !== "");

    const groups = GEM_GROUP_ORDER.map((group) => ({
      group,
      nodes: boardFlags
        .filter((flag) => {
          const node = MasterTraits[toHashString(flag.hash)];
          return node.cost === 50 && node.group === group;
        })
        .map(toDetail)
        .filter((node) => node.text !== ""),
    })).filter((rankGroup) => rankGroup.nodes.length > 0);

    return { board, cardHash, perks, groups };
  }).filter((block) => block.cardHash !== null || block.perks.length > 0 || block.groups.length > 0);
};
