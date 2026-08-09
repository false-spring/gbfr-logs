import { describe, expect, it, vi } from "vitest";

const { CATALOG } = vi.hoisted(() => {
  const node = (board: string, group: string, cost: number, card: boolean) => ({ board, group, cost, card });

  return {
    CATALOG: {
      "3785ee72": node("SB_DEF", "rank1", 100, true),
      "00000001": node("SB_DEF", "rank2", 100, false),
      "00000002": node("SB_DEF", "rank3", 100, false),
      "00000010": node("SB_DEF", "rank1", 50, false),
      "00000011": node("SB_DEF", "EX", 50, false),
      "00000020": node("SB_ATK", "rank1", 100, true),
      "00000021": node("SB_ATK", "rank1", 50, false),
      "00000030": node("SB_LIMIT", "rank1", 100, true),
      "00000031": node("SB_LIMIT", "rank1", 50, false),
    },
  };
});
vi.mock("@/assets/master-traits", () => ({ default: CATALOG }));

const CARD_NAMES: Record<string, string> = {
  "mastertraits:3785ee72.text": "Insight: Bulwark",
  "mastertraits:00000020.text": "Essence: Onslaught",
  "mastertraits:00000030.text": "Crux: Overdrive",
};
vi.mock("i18next", () => ({
  t: (key: string | string[]) => {
    const keys = Array.isArray(key) ? key : [key];
    return CARD_NAMES[keys[0]] ?? keys[keys.length - 1];
  },
}));

import { type MasterTraitFlag } from "@/types";

import {
  boardGemCounts,
  masteryLevel,
  styleBoardName,
  styleBoardSourceName,
  styleBoardSummaries,
  styleRankDiamonds,
} from "./skillboard";

const flag = (hash: number, on: boolean): MasterTraitFlag => ({ hash, on });

const emptyBand = (): MasterTraitFlag[] => Object.keys(CATALOG).map((hash) => flag(parseInt(hash, 16), false));

describe("styleBoardSummaries", () => {
  it("keeps a board whose gems are allocated but whose style card is unpurchased", () => {
    const flags = [
      flag(0x3785ee72, false), // SB_DEF card — NOT purchased
      flag(0x00000010, true), // ...but a rank1 gem is allocated
      flag(0x00000020, true),
      flag(0x00000021, true),
    ];

    const summaries = styleBoardSummaries(flags);
    expect(summaries[0]).toEqual({
      board: "SB_DEF",
      cardHash: 0x3785ee72,
      ranksOn: 0,
      ranksTotal: 1,
      gems: [1, 0, 0, 0],
    });
    expect(summaries[1]).toMatchObject({ board: "SB_ATK", cardHash: 0x00000020, ranksOn: 1 });
  });

  it("always returns all three boards in board order, even untouched ones", () => {
    expect(styleBoardSummaries(emptyBand()).map((entry) => entry.board)).toEqual(["SB_DEF", "SB_ATK", "SB_LIMIT"]);
    expect(styleBoardSummaries([flag(0x00000031, true)]).map((entry) => entry.board)).toEqual([
      "SB_DEF",
      "SB_ATK",
      "SB_LIMIT",
    ]);
  });

  it("zeroes an untouched board without hiding it", () => {
    expect(styleBoardSummaries(emptyBand())[2]).toEqual({
      board: "SB_LIMIT",
      cardHash: 0x00000030,
      ranksOn: 0,
      ranksTotal: 1,
      gems: [0, 0, 0, 0],
    });
  });

  it("counts every allocated style-rank row, not just the card", () => {
    const flags = [flag(0x3785ee72, true), flag(0x00000001, true), flag(0x00000002, false)];

    expect(styleBoardSummaries(flags)[0]).toMatchObject({ ranksOn: 2, ranksTotal: 3 });
  });

  it("reports a null cardHash when the flag band carries no card row for the board", () => {
    expect(styleBoardSummaries([flag(0x00000010, true)])[0]).toMatchObject({ cardHash: null, ranksTotal: 0 });
  });

  it("ignores hashes absent from the catalog", () => {
    const flags = [flag(0xdeadbeef, true), flag(0x00000020, true), flag(0x00000021, true)];

    expect(styleBoardSummaries(flags)[1]).toEqual({
      board: "SB_ATK",
      cardHash: 0x00000020,
      ranksOn: 1,
      ranksTotal: 1,
      gems: [1, 0, 0, 0],
    });
  });
});

describe("styleRankDiamonds", () => {
  it("fills one diamond per allocated style-rank row", () => {
    expect(styleRankDiamonds(0, 3)).toBe("◇◇◇");
    expect(styleRankDiamonds(1, 3)).toBe("◆◇◇");
    expect(styleRankDiamonds(2, 3)).toBe("◆◆◇");
    expect(styleRankDiamonds(3, 3)).toBe("◆◆◆");
  });

  it("renders nothing when the band carries no style-rank rows", () => {
    expect(styleRankDiamonds(0, 0)).toBe("");
  });

  it("uses the Geometric Shapes pair, which has no emoji presentation", () => {
    const pips = styleRankDiamonds(1, 2);
    expect([...pips].map((c) => c.codePointAt(0))).toEqual([0x25c6, 0x25c7]);
    expect(pips).not.toMatch(/[♠-♧]/);
  });
});

describe("styleBoardName", () => {
  it("names a board from its style card even when the card is unpurchased", () => {
    expect(styleBoardName(0x3785ee72, "SB_DEF")).toBe("Insight: Bulwark");
  });

  it("falls back to the generic style name when there is no card row", () => {
    expect(styleBoardName(null, "SB_DEF")).toBe("Insight");
    expect(styleBoardName(null, "SB_ATK")).toBe("Essence");
    expect(styleBoardName(null, "SB_LIMIT")).toBe("Crux");
  });
});

describe("styleBoardSourceName", () => {
  // 9997 is the board, not any one effect, so it means a different card for
  // every character carrying it — a raw `#9997` was all it could render before.
  it("names each of the three board source ids from the applier's own band", () => {
    const band = emptyBand();

    expect(styleBoardSourceName(9999, band)).toBe("Insight: Bulwark");
    expect(styleBoardSourceName(9998, band)).toBe("Essence: Onslaught");
    expect(styleBoardSourceName(9997, band)).toBe("Crux: Overdrive");
  });

  it("names the board even when its style card is unpurchased", () => {
    // The card row gates Style Rank 2/3, so an unpurchased card still names the
    // board the game shows.
    expect(styleBoardSourceName(9997, [flag(0x00000030, false)])).toBe("Crux: Overdrive");
  });

  it("falls back to the generic style name for a band with no card row", () => {
    expect(styleBoardSourceName(9997, [])).toBe("Crux");
    expect(styleBoardSourceName(9997, undefined)).toBe("Crux");
  });

  it("leaves every other source id to the tables that name effects", () => {
    expect(styleBoardSourceName(9996, emptyBand())).toBeNull();
    expect(styleBoardSourceName(10000, emptyBand())).toBeNull();
  });
});

describe("gem counting", () => {
  const flags = [
    flag(0x3785ee72, false), // card rows are cost 100 — never counted
    flag(0x00000010, true),
    flag(0x00000011, true),
    flag(0x00000021, true),
    flag(0x00000031, false),
  ];

  it("counts allocated gems per rank group", () => {
    expect(boardGemCounts(flags, "SB_DEF")).toEqual([1, 0, 0, 1]);
    expect(boardGemCounts(flags, "SB_ATK")).toEqual([1, 0, 0, 0]);
    expect(boardGemCounts(flags, "SB_LIMIT")).toEqual([0, 0, 0, 0]);
  });

  it("sums allocated gems across boards for the mastery level", () => {
    expect(masteryLevel(flags)).toBe(3);
  });
});
