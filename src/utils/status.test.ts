import { describe, expect, it } from "vitest";

import StatusCatalog from "@/assets/status-catalog";
import StatusClassNames from "@/assets/status-class-names";

import { type StatusIntervals, type StatusPeakStacks, type StatusSourceWindows, type StatusStackSeries } from "@/types";
import {
  STATUS_SOURCE_ALL,
  buildStatusEntityOptions,
  buildStatusOptionGroups,
  buildStatusSourceOptions,
  formatStatusValue,
  masterTraitSourceFor,
  resolveSelectedOption,
  selectedStackSamples,
  selectedStatusWindows,
  statusDisplayName,
  statusLabelWithStacks,
  statusPointsForChart,
  statusPolarity,
  statusSourceKey,
} from "@/utils/status";

const GUTS = 20;
const REGEN = 5;
const ATK_DOWN = 2;
const UNCATALOGUED = 0x270f;

const FERRY = 0x80000000;
const CAGLIOSTRO = 0x80000002;
const BOSS = 0x1234abcd;
const OTHER_ENEMY = 0x0badf00d;

const label = (actorId: number): string =>
  ({ [FERRY]: "[1] Ferry", [CAGLIOSTRO]: "[3] Cagliostro", [BOSS]: "Managarmr", [OTHER_ENEMY]: "Bahamut" })[actorId] ??
  "?";

const intervals: StatusIntervals = {
  [CAGLIOSTRO]: { [GUTS]: [[0, 5_000]] },
  [FERRY]: {
    [REGEN]: [[1_000, 4_000]],
    [ATK_DOWN]: [[8_000, 9_000]],
    [UNCATALOGUED]: [[2_000, 3_000]],
  },
  [BOSS]: { [ATK_DOWN]: [[3_000, 12_000]] },
  [OTHER_ENEMY]: { [GUTS]: [[0, 1_000]] },
};

describe("statusDisplayName", () => {
  it("names a catalogued status", () => {
    expect(statusDisplayName(GUTS)).toBe("Guts");
  });

  it("prefers a hand-written label for a kind the table leaves unnamed", () => {
    expect(statusDisplayName(0x0c)).toBe("Manigance");
    expect(statusDisplayName(0x3fe)).toBe("Debuff Extension");
  });

  it("falls back to the class name where nobody has named it yet", () => {
    expect(statusDisplayName(0x22)).toBe("StatusEm1700AttackBuff");
    expect(statusDisplayName(0x3f6)).toBe("StatusAilmentProvocation");
  });

  it("groups a named fallback with the buffs or ailments, not uncatalogued", () => {
    expect(statusPolarity(0x10)).toBe("buff");
    expect(statusPolarity(0x3fe)).toBe("debuff");
    expect(statusPolarity(0x22)).toBe("other");
  });

  it("still falls back to hex for an id this build has never heard of", () => {
    expect(statusDisplayName(UNCATALOGUED)).toBe("Status 0x270f");
  });

  it("prefers the game's own name over the class name", () => {
    expect(statusDisplayName(GUTS)).toBe("Guts");
  });
});

describe("buildStatusEntityOptions", () => {
  it("lists party members first in slot order, then everything else by name", () => {
    expect(buildStatusEntityOptions(intervals, label)).toEqual([
      { value: String(FERRY), label: "[1] Ferry" },
      { value: String(CAGLIOSTRO), label: "[3] Cagliostro" },
      { value: String(OTHER_ENEMY), label: "Bahamut" },
      { value: String(BOSS), label: "Managarmr" },
    ]);
  });

  it("never lists an actor with no intervals behind it", () => {
    const sparse: StatusIntervals = { [FERRY]: { [GUTS]: [] }, [BOSS]: {} };

    expect(buildStatusEntityOptions(sparse, label)).toEqual([]);
  });

  it("renders nothing for a log recorded without the status hooks", () => {
    expect(buildStatusEntityOptions({}, label)).toEqual([]);
  });
});

describe("buildStatusOptionGroups", () => {
  it("groups one actor's statuses buffs-first, alphabetical inside each group", () => {
    expect(buildStatusOptionGroups(intervals, FERRY)).toEqual([
      { polarity: "buff", items: [{ value: String(REGEN), label: "Regen" }] },
      { polarity: "debuff", items: [{ value: String(ATK_DOWN), label: "ATK↓" }] },
      { polarity: "other", items: [{ value: String(UNCATALOGUED), label: "Status 0x270f" }] },
    ]);
  });

  it("drops the groups an actor has nothing in", () => {
    expect(buildStatusOptionGroups(intervals, BOSS)).toEqual([
      { polarity: "debuff", items: [{ value: String(ATK_DOWN), label: "ATK↓" }] },
    ]);
  });

  it("lists nothing until an actor is picked, or for one that holds nothing", () => {
    expect(buildStatusOptionGroups(intervals, null)).toEqual([]);
    expect(buildStatusOptionGroups(intervals, 0x99)).toEqual([]);
  });

  it("annotates a stacked status and leaves every other label alone", () => {
    const peaks: StatusPeakStacks = { [FERRY]: { [REGEN]: 3, [ATK_DOWN]: 1 } };

    expect(buildStatusOptionGroups(intervals, FERRY, peaks)).toEqual([
      { polarity: "buff", items: [{ value: String(REGEN), label: "Regen (×3)" }] },
      { polarity: "debuff", items: [{ value: String(ATK_DOWN), label: "ATK↓" }] },
      { polarity: "other", items: [{ value: String(UNCATALOGUED), label: "Status 0x270f" }] },
    ]);
  });
});

describe("statusLabelWithStacks", () => {
  it("shows the peak depth once a status stacked", () => {
    expect(statusLabelWithStacks(GUTS, 3)).toBe("Guts (×3)");
  });

  it("leaves a status that never stacked unannotated", () => {
    expect(statusLabelWithStacks(GUTS, 1)).toBe("Guts");
    expect(statusLabelWithStacks(GUTS, 0)).toBe("Guts");
    expect(statusLabelWithStacks(GUTS, undefined)).toBe("Guts");
  });
});

describe("selectedStatusWindows", () => {
  it("returns the picked actor's windows for the picked status", () => {
    expect(selectedStatusWindows(intervals, BOSS, ATK_DOWN)).toEqual([[3_000, 12_000]]);
  });

  it("returns nothing until both selectors are set", () => {
    expect(selectedStatusWindows(intervals, BOSS, null)).toEqual([]);
    expect(selectedStatusWindows(intervals, null, ATK_DOWN)).toEqual([]);
    expect(selectedStatusWindows(intervals, FERRY, GUTS)).toEqual([]);
  });
});

describe("selectedStackSamples", () => {
  const FERRY = 0x80000000;
  const GUTS = 20;
  const series: StatusStackSeries = { [FERRY]: { [GUTS]: [[0, 3]] } };

  it("returns the selected status's samples", () => {
    expect(selectedStackSamples(series, FERRY, GUTS)).toEqual([[0, 3]]);
  });

  it("is empty unless both selectors are set", () => {
    expect(selectedStackSamples(series, null, GUTS)).toEqual([]);
    expect(selectedStackSamples(series, FERRY, null)).toEqual([]);
  });

  it("is empty for a status that never stacked", () => {
    expect(selectedStackSamples(series, FERRY, 999)).toEqual([]);
  });
});

describe("StatusClassNames", () => {
  it("never shadows a status the game does name", () => {
    const overlap = Object.keys(StatusClassNames)
      .map(Number)
      .filter((id) => StatusCatalog[id]?.name);

    expect(overlap).toEqual([]);
  });
});

describe("statusPointsForChart", () => {
  const EXTENT = 15_000;

  it("draws one point per transition, at its exact millisecond", () => {
    const samples: [number, number][] = [
      [4_300, 0.1],
      [4_827, 0.2],
      [13_239, 0.0],
    ];

    expect(statusPointsForChart(samples, EXTENT)).toEqual([
      { t: 4_300, value: 0.1 },
      { t: 4_827, value: 0.2 },
      { t: 13_239, value: 0 },
      { t: 15_000, value: 0 },
    ]);
  });

  it("runs the last value out to the chart's edge", () => {
    expect(statusPointsForChart([[2_000, 0.5]], EXTENT)).toEqual([
      { t: 2_000, value: 0.5 },
      { t: 15_000, value: 0.5 },
    ]);
  });

  it("starts where the status did, not at the chart's left edge", () => {
    const points = statusPointsForChart([[2_000, 0.5]], EXTENT);

    expect(points[0].t).toBe(2_000);
  });

  it("collapses a flurry inside one millisecond to what stood at its end", () => {
    const samples: [number, number][] = [
      [3_000, 1],
      [3_000, 3],
    ];

    expect(statusPointsForChart(samples, EXTENT)).toEqual([
      { t: 3_000, value: 3 },
      { t: 15_000, value: 3 },
    ]);
  });

  it("keeps a distinct point for changes milliseconds apart", () => {
    const samples: [number, number][] = [
      [3_000, 1],
      [3_028, 3],
    ];

    expect(statusPointsForChart(samples, EXTENT)).toEqual([
      { t: 3_000, value: 1 },
      { t: 3_028, value: 3 },
      { t: 15_000, value: 3 },
    ]);
  });

  it("skips a repeat, which draws no step", () => {
    const samples: [number, number][] = [
      [1_000, 0.2],
      [5_000, 0.2],
      [9_000, 0.4],
    ];

    expect(statusPointsForChart(samples, EXTENT)).toEqual([
      { t: 1_000, value: 0.2 },
      { t: 9_000, value: 0.4 },
      { t: 15_000, value: 0.4 },
    ]);
  });

  it("renders nothing for an empty chart or an empty status", () => {
    expect(statusPointsForChart([[0, 3]], 0)).toEqual([]);
    expect(statusPointsForChart([], EXTENT)).toEqual([]);
  });
});

describe("formatStatusValue", () => {
  it("renders a fractional magnitude as a percentage", () => {
    expect(formatStatusValue(0.3, true)).toBe("30.0%");
    expect(formatStatusValue(0.075, true)).toBe("7.5%");
  });

  it("leaves a count alone", () => {
    expect(formatStatusValue(89901, false)).toBe("89,901");
    expect(formatStatusValue(4499, false)).toBe("4,499");
  });

  it("falls back to the plain number when the units are unknown", () => {
    expect(formatStatusValue(0.3, undefined)).toBe("0.3");
  });

  it("shows magnitude, not direction", () => {
    expect(formatStatusValue(0.2, true)).toBe("20.0%");
  });
});

describe("resolveSelectedOption", () => {
  const aggregate = { value: STATUS_SOURCE_ALL, label: "All" };
  const ferry = { value: "2147483648:0,7,0", label: "[1] Ferry" };
  const cagliostro = { value: "2147483650:0,9,0", label: "[3] Cagliostro" };

  it("keeps a selection the rebuilt list still offers", () => {
    expect(resolveSelectedOption([aggregate, ferry, cagliostro], ferry.value)).toBe(ferry.value);
  });

  it("takes the topmost option when the selection is gone", () => {
    expect(resolveSelectedOption([aggregate, ferry], cagliostro.value)).toBe(STATUS_SOURCE_ALL);
  });

  it("takes the only source when a status has just one, where there is no aggregate", () => {
    expect(resolveSelectedOption([ferry], STATUS_SOURCE_ALL)).toBe(ferry.value);
  });

  it("falls back to the aggregate when there is nothing to choose from", () => {
    expect(resolveSelectedOption([], ferry.value)).toBe(STATUS_SOURCE_ALL);
  });
});

describe("buildStatusSourceOptions", () => {
  const source = (applierIndex: number | null, actionId: number | null): StatusSourceWindows => ({
    applierIndex,
    sourceIds: actionId === null ? null : [0, actionId, 0],
    windows: [[0, 1_000]],
    values: [],
  });

  it("offers the aggregate first when there is more than one source", () => {
    const options = buildStatusSourceOptions([source(FERRY, 7), source(CAGLIOSTRO, 9)], label, "All", "Unknown");

    expect(options[0]).toEqual({ value: STATUS_SOURCE_ALL, label: "All" });
  });

  it("omits the aggregate when there is only one source", () => {
    const options = buildStatusSourceOptions([source(FERRY, 7)], label, "All", "Unknown");

    expect(options.map((option) => option.value)).not.toContain(STATUS_SOURCE_ALL);
    expect(options).toHaveLength(1);
  });

  it("names each source by its character, with the raw id when unnamed", () => {
    const options = buildStatusSourceOptions([source(FERRY, 7), source(CAGLIOSTRO, 9)], label, "All", "Unknown");

    expect(options.map((option) => option.label)).toEqual(["All", "[1] Ferry (#7)", "[3] Cagliostro (#9)"]);
  });

  it("shows the bare character when there was no action at all", () => {
    const options = buildStatusSourceOptions([source(FERRY, 0), source(CAGLIOSTRO, 0)], label, "All", "Unknown");

    expect(options.map((option) => option.label)).toEqual(["All", "[1] Ferry", "[3] Cagliostro"]);
  });

  const detailed = (
    applierIndex: number,
    sourceIds: [number, number, number],
    magnitude: number
  ): StatusSourceWindows => ({
    applierIndex,
    sourceIds,
    windows: [[0, 1_000]],
    values: [
      [0, 0],
      [10, magnitude],
    ],
  });

  it("does not print an action id that is only the applier's own identity", () => {
    const options = buildStatusSourceOptions([detailed(BOSS, [0x028000, 0x8000, 0], 0.1)], label, "All", "Unknown");

    expect(options.map((option) => option.label)).toEqual(["Managarmr"]);
  });

  it("tells one applier's unnamed sources apart by magnitude rather than by counting", () => {
    const options = buildStatusSourceOptions(
      [detailed(BOSS, [0, 0, 0], 0.2), detailed(BOSS, [0, 0, 1], 0.3)],
      label,
      "All",
      "Unknown",
      undefined,
      true
    );

    expect(options.map((option) => option.label)).toEqual(["All", "Managarmr (20%)", "Managarmr (30%)"]);
  });

  it("falls back to counting when the magnitudes do not tell the sources apart", () => {
    const options = buildStatusSourceOptions(
      [detailed(BOSS, [0, 0, 0], 0.2), detailed(BOSS, [0, 0, 1], 0.2)],
      label,
      "All",
      "Unknown",
      undefined,
      true
    );

    expect(options.map((option) => option.label)).toEqual(["All", "Managarmr (1)", "Managarmr (2)"]);
  });

  it("separates one character's entries by their ids, not by position", () => {
    const options = buildStatusSourceOptions(
      [source(FERRY, 10_000), source(FERRY, 10_001), source(CAGLIOSTRO, 12_000)],
      label,
      "All",
      "Unknown"
    );

    expect(options.map((option) => option.label)).toEqual([
      "All",
      "[1] Ferry (#10000)",
      "[1] Ferry (#10001)",
      "[3] Cagliostro (#12000)",
    ]);
  });

  it("names the ability when the source id resolves to one", () => {
    const resolveAction = (_applier: number, actionId: number) =>
      ({ 1000: "Königsschild", 1400: "Salvator" })[actionId] ?? null;

    const options = buildStatusSourceOptions(
      [source(FERRY, 1400), source(CAGLIOSTRO, 1000)],
      label,
      "All",
      "Unknown",
      resolveAction
    );

    expect(options.map((option) => option.label)).toEqual([
      "All",
      "[1] Ferry: Salvator",
      "[3] Cagliostro: Königsschild",
    ]);
  });

  it("marks an unconfirmed guess so it cannot read as settled", () => {
    const resolveAction = (_applier: number, actionId: number) =>
      actionId === 1400 ? "Salvator" : actionId === 2000 ? "Versalis Ignition?" : null;

    const options = buildStatusSourceOptions(
      [source(FERRY, 1400), source(CAGLIOSTRO, 2000)],
      label,
      "All",
      "Unknown",
      resolveAction
    );

    expect(options.map((option) => option.label)).toEqual([
      "All",
      "[1] Ferry: Salvator",
      "[3] Cagliostro: Versalis Ignition?",
    ]);
  });

  it("shows an unresolved id rather than hiding it", () => {
    const options = buildStatusSourceOptions([source(FERRY, 99999)], label, "All", "Unknown", () => null);

    expect(options.map((option) => option.label)).toEqual(["[1] Ferry (#99999)"]);
  });

  it("does not number entries that are already named apart", () => {
    const resolveAction = (_applier: number, actionId: number) =>
      ({ 1400: "Salvator", 1500: "Mirage" })[actionId] ?? null;

    const options = buildStatusSourceOptions(
      [source(FERRY, 1400), source(FERRY, 1500)],
      label,
      "All",
      "Unknown",
      resolveAction
    );

    expect(options.map((option) => option.label)).toEqual(["All", "[1] Ferry: Salvator", "[1] Ferry: Mirage"]);
  });

  it("shows an unresolved source rather than dropping it", () => {
    const options = buildStatusSourceOptions([source(null, null)], label, "All", "Unknown");

    expect(options.map((option) => option.label)).toEqual(["Unknown"]);
  });

  it("keys entries distinctly even when one half is absent", () => {
    expect(statusSourceKey(source(FERRY, null))).not.toBe(statusSourceKey(source(null, null)));
    expect(statusSourceKey(source(FERRY, 7))).not.toBe(statusSourceKey(source(FERRY, 9)));
  });
});

describe("masterTraitSourceFor", () => {
  const SHIELD = 41;

  it("names the perk when the magnitude matches what the node declares", () => {
    expect(masterTraitSourceFor("PL0600", SHIELD, 1000, false)).toBe("Upon performing a perfect dodge");
  });

  it("does NOT claim a shield that came from somewhere else", () => {
    for (const wrong of [79_135, 33_331, 87_235, 99_999]) {
      expect(masterTraitSourceFor("PL0600", SHIELD, wrong, false)).toBeNull();
    }
  });

  it("converts the table's percent to the meter's fraction", () => {
    const perk = masterTraitSourceFor("PL0400", 1, 0.1, true);
    expect(perk).toBe("Mystic Vortex");
    expect(masterTraitSourceFor("PL0400", 1, 10, true)).toBeNull();
  });

  it("returns null for a character or kind with no master-trait source", () => {
    expect(masterTraitSourceFor("PL0600", 0x999, 1000, false)).toBeNull();
    expect(masterTraitSourceFor(undefined, SHIELD, 1000, false)).toBeNull();
    expect(masterTraitSourceFor("PL0600", SHIELD, undefined, false)).toBeNull();
  });

  it("names every node that declares the value, not just the first", () => {
    const ATK_DOWN = 2;
    expect(masterTraitSourceFor("Pl1800", ATK_DOWN, 0.09999999403953552, true)).toBe(
      "Disruption / Insight: Crippling Combos"
    );
  });

  it("names a rank card even though its status carries no magnitude", () => {
    const TRUTH = 92;
    expect(masterTraitSourceFor("Pl1800", TRUTH, undefined, undefined)).toBe("Insight: Crippling Combos Rank 3");
  });

  it("does not treat a node's precondition as something it grants", () => {
    expect(masterTraitSourceFor("Pl2500", SHIELD, 5, false)).toBeNull();
  });
});
