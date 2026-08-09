import { type LinkTimeWindow } from "@/types";
import { describe, expect, it } from "vitest";
import {
  MIN_OVERLAY_SPAN,
  breakAreas,
  confluxBossLines,
  confluxSeamBuckets,
  confluxSeamLines,
  linkTimeAreas,
  overlayBands,
  overlaySpans,
  overlayTrackSegments,
  rollingWindowStart,
  sbaLockdownAreas,
  statusAreas,
} from "./chartCommon";

const DPS_MS = 3_000;

const DPS_BUCKETS = 41;
const DPS_SPAN_MS = 120_000;

describe("overlayBands", () => {
  it("snaps a window onto the buckets it covers, rounding the end up", () => {
    expect(overlayBands([[4_500, 13_500]], DPS_MS, 20)).toEqual([{ startIndex: 1, endIndex: 5 }]);
  });

  it("still shades a full bucket for a window narrower than one", () => {
    expect(overlayBands([[3_100, 3_400]], DPS_MS, 20)).toEqual([{ startIndex: 1, endIndex: 2 }]);
  });

  it("merges windows that touch or overlap after rounding", () => {
    expect(
      overlayBands(
        [
          [0, 6_000],
          [6_100, 12_000],
        ],
        DPS_MS,
        20
      )
    ).toEqual([{ startIndex: 0, endIndex: 4 }]);
  });

  it("orders bands by start regardless of the input order", () => {
    expect(
      overlayBands(
        [
          [30_000, 36_000],
          [3_000, 9_000],
        ],
        DPS_MS,
        20
      )
    ).toEqual([
      { startIndex: 1, endIndex: 3 },
      { startIndex: 10, endIndex: 12 },
    ]);
  });

  it("clamps a window running past the chart and drops one starting past it", () => {
    expect(overlayBands([[24_000, 60_000]], DPS_MS, 10)).toEqual([{ startIndex: 8, endIndex: 9 }]);
    expect(overlayBands([[40_000, 60_000]], DPS_MS, 10)).toEqual([]);
  });

  it("renders nothing for an uncaptured encounter or a chart with no span", () => {
    expect(overlayBands([], DPS_MS, 20)).toEqual([]);
    expect(overlayBands([[0, 6_000]], DPS_MS, 1)).toEqual([]);
  });

  it("uses the SBA chart's own 1s buckets", () => {
    expect(overlayBands([[4_500, 13_500]], 1_000, 60)).toEqual([{ startIndex: 4, endIndex: 14 }]);
  });
});

describe("overlaySpans", () => {
  it("renders a sub-bucket window narrower than a bucket instead of filling one", () => {
    const [span] = overlaySpans([[3_100, 3_500]], DPS_MS, DPS_BUCKETS);

    expect(span).toEqual({ left: 3_100 / DPS_SPAN_MS, width: 400 / DPS_SPAN_MS });
    expect(span.width).toBeLessThan(DPS_MS / DPS_SPAN_MS);
    expect(overlayBands([[3_100, 3_500]], DPS_MS, DPS_BUCKETS)).toEqual([{ startIndex: 1, endIndex: 2 }]);
  });

  it("places a window covering the encounter edge to edge", () => {
    expect(overlaySpans([[0, DPS_SPAN_MS]], DPS_MS, DPS_BUCKETS)).toEqual([{ left: 0, width: 1 }]);
  });

  it("floors a window too short to paint a pixel", () => {
    const [span] = overlaySpans([[6_000, 6_050]], DPS_MS, DPS_BUCKETS);

    expect(span.left).toBe(6_000 / DPS_SPAN_MS);
    expect(span.width).toBeCloseTo(MIN_OVERLAY_SPAN, 12);
  });

  it("floors inward at the right edge rather than overflowing the plot", () => {
    const [span] = overlaySpans([[DPS_SPAN_MS - 50, DPS_SPAN_MS]], DPS_MS, DPS_BUCKETS);

    expect(span.width).toBeCloseTo(MIN_OVERLAY_SPAN, 12);
    expect(span.left + span.width).toBeCloseTo(1, 12);
  });

  it("drops a zero-length window rather than flooring it into view", () => {
    expect(overlaySpans([[6_000, 6_000]], DPS_MS, DPS_BUCKETS)).toEqual([]);
    expect(overlaySpans([[6_000, 5_000]], DPS_MS, DPS_BUCKETS)).toEqual([]);
  });

  it("merges windows that overlap", () => {
    expect(
      overlaySpans(
        [
          [0, 9_000],
          [6_000, 12_000],
        ],
        DPS_MS,
        DPS_BUCKETS
      )
    ).toEqual([{ left: 0, width: 12_000 / DPS_SPAN_MS }]);
  });

  it("merges neighbours that only meet once floored, so the fills never stack", () => {
    expect(
      overlaySpans(
        [
          [1_000, 1_010],
          [1_100, 1_110],
        ],
        DPS_MS,
        DPS_BUCKETS
      )
    ).toEqual([{ left: 1_000 / DPS_SPAN_MS, width: (100 + MIN_OVERLAY_SPAN * DPS_SPAN_MS) / DPS_SPAN_MS }]);
  });

  it("orders spans by start regardless of the input order", () => {
    expect(
      overlaySpans(
        [
          [30_000, 36_000],
          [3_000, 9_000],
        ],
        DPS_MS,
        DPS_BUCKETS
      )
    ).toEqual([
      { left: 3_000 / DPS_SPAN_MS, width: 6_000 / DPS_SPAN_MS },
      { left: 30_000 / DPS_SPAN_MS, width: 6_000 / DPS_SPAN_MS },
    ]);
  });

  it("clamps a window running past the chart and drops one starting past it", () => {
    expect(overlaySpans([[114_000, 180_000]], DPS_MS, DPS_BUCKETS)).toEqual([
      { left: 114_000 / DPS_SPAN_MS, width: 6_000 / DPS_SPAN_MS },
    ]);
    expect(overlaySpans([[150_000, 180_000]], DPS_MS, DPS_BUCKETS)).toEqual([]);
  });

  it("renders nothing for an uncaptured overlay or a chart with no span", () => {
    expect(overlaySpans([], DPS_MS, DPS_BUCKETS)).toEqual([]);
    expect(overlaySpans([[0, 6_000]], DPS_MS, 1)).toEqual([]);
    expect(overlaySpans([[0, 6_000]], 0, DPS_BUCKETS)).toEqual([]);
  });

  it("uses the SBA chart's own 1s buckets", () => {
    expect(overlaySpans([[3_100, 3_500]], 1_000, 61)).toEqual([{ left: 3_100 / 60_000, width: 400 / 60_000 }]);
  });
});

describe("confluxSeamBuckets", () => {
  it("rounds each area clear UP to a bucket", () => {
    expect(confluxSeamBuckets([9_000, 30_400], DPS_MS, 20)).toEqual([3, 11]);
  });

  it("collapses two clears that land in the same bucket", () => {
    expect(confluxSeamBuckets([8_500, 9_000], DPS_MS, 20)).toEqual([3]);
  });

  it("returns them in order however they arrive", () => {
    expect(confluxSeamBuckets([30_000, 9_000, 18_000], DPS_MS, 20)).toEqual([3, 6, 10]);
  });

  it("uses the SBA chart's own 1s buckets", () => {
    expect(confluxSeamBuckets([9_000, 30_400], 1_000, 60)).toEqual([9, 31]);
  });

  it("drops seams on the chart's own edges", () => {
    expect(confluxSeamBuckets([0, 57_000, 60_000], DPS_MS, 20)).toEqual([]);
    expect(confluxSeamBuckets([1_000], DPS_MS, 20)).toEqual([1]);
  });

  it("renders nothing for a non-Conflux log or a chart with no span", () => {
    expect(confluxSeamBuckets([], DPS_MS, 20)).toEqual([]);
    expect(confluxSeamBuckets([9_000], DPS_MS, 1)).toEqual([]);
  });
});

describe("confluxSeamLines", () => {
  it("names the y axis every rule is drawn against", () => {
    const lines = confluxSeamLines([9_000, 18_000], DPS_MS, 20);

    expect(lines).toHaveLength(2);
    for (const line of lines) {
      expect(line.props.yAxisId).toBe("left");
    }
  });
});

describe("reference overlays name their y axis", () => {
  it("shades Link Time against the left axis", () => {
    const areas = linkTimeAreas([[9_000, 30_000]], DPS_MS, 40);

    expect(areas).toHaveLength(1);
    expect(areas[0].props.yAxisId).toBe("left");
  });
});

describe("statusAreas", () => {
  it("shades a status in a colour no other overlay uses, and more heavily", () => {
    const [status] = statusAreas([[9_000, 30_000]], DPS_MS, 40);
    const [link] = linkTimeAreas([[9_000, 30_000]], DPS_MS, 40);
    const [sba] = sbaLockdownAreas([[9_000, 30_000]], DPS_MS, 40);
    const [brk] = breakAreas([[9_000, 30_000]], DPS_MS, 40);

    expect(status.props.yAxisId).toBe("left");
    expect([link.props.fill, sba.props.fill, brk.props.fill]).not.toContain(status.props.fill);
    expect(status.props.fillOpacity).toBeGreaterThan(link.props.fillOpacity);
  });

  it("renders nothing when no status is selected", () => {
    expect(statusAreas([], DPS_MS, 40)).toEqual([]);
  });

  it("draws its own rects from raw milliseconds instead of snapping to buckets", () => {
    const [area] = statusAreas([[3_100, 3_500]], DPS_MS, DPS_BUCKETS);

    expect(area.props.x1).toBeUndefined();
    expect(area.props.x2).toBeUndefined();
    expect(area.props.shape.props.spans).toEqual(overlaySpans([[3_100, 3_500]], DPS_MS, DPS_BUCKETS));
    expect(area.props.shape.props.spans[0].width).toBeLessThan(DPS_MS / DPS_SPAN_MS);
  });

  it("carries every span in one area, so the fill and the clip resolve once", () => {
    const areas = statusAreas(
      [
        [3_000, 9_000],
        [30_000, 36_000],
      ],
      DPS_MS,
      DPS_BUCKETS
    );

    expect(areas).toHaveLength(1);
    expect(areas[0].props.shape.props.spans).toHaveLength(2);
    expect(areas[0].props.ifOverflow).toBe("hidden");
  });
});

describe("overlayTrackSegments", () => {
  it("places a window as a percentage span of the plot width", () => {
    expect(overlayTrackSegments([[9_000, 15_000]], DPS_MS, 11)).toEqual([{ left: 30, width: 20 }]);
  });

  it("spans the whole track for a window covering the encounter", () => {
    expect(overlayTrackSegments([[0, 30_000]], DPS_MS, 11)).toEqual([{ left: 0, width: 100 }]);
  });

  it("positions from raw milliseconds rather than snapping to the chart's buckets", () => {
    expect(overlayBands([[4_500, 13_500]], 1_000, 61)).toEqual([{ startIndex: 4, endIndex: 14 }]);
    expect(overlayTrackSegments([[4_500, 13_500]], 1_000, 61)).toEqual([
      { left: (4_500 / 60_000) * 100, width: (9_000 / 60_000) * 100 },
    ]);
  });

  it("keeps a sub-bucket window narrower than a bucket, and still visible", () => {
    const [narrow] = overlayTrackSegments([[3_100, 3_500]], DPS_MS, DPS_BUCKETS);
    const [tiny] = overlayTrackSegments([[3_100, 3_150]], DPS_MS, DPS_BUCKETS);

    expect(narrow.width).toBeLessThan((DPS_MS / DPS_SPAN_MS) * 100);
    expect(tiny.width).toBeCloseTo(MIN_OVERLAY_SPAN * 100, 10);
  });

  it("orders and merges exactly as the in-plot spans do", () => {
    const windows: LinkTimeWindow[] = [
      [30_000, 36_000],
      [3_000, 9_000],
      [8_000, 12_000],
    ];

    expect(overlayTrackSegments(windows, DPS_MS, DPS_BUCKETS)).toEqual(
      overlaySpans(windows, DPS_MS, DPS_BUCKETS).map(({ left, width }) => ({ left: left * 100, width: width * 100 }))
    );
    expect(overlayTrackSegments(windows, DPS_MS, DPS_BUCKETS)).toHaveLength(2);
  });

  it("renders nothing for an uncaptured overlay or a chart with no span", () => {
    expect(overlayTrackSegments([], DPS_MS, 11)).toEqual([]);
    expect(overlayTrackSegments([[0, 6_000]], DPS_MS, 1)).toEqual([]);
  });
});

describe("rollingWindowStart", () => {
  it("clamps the window to the most recent seam", () => {
    const seams = [10, 25];

    expect(rollingWindowStart(12, seams)).toBe(10);
    expect(rollingWindowStart(30, seams)).toBe(25);
  });

  it("starts the window at the seam itself, so that bucket smooths over nothing", () => {
    expect(rollingWindowStart(10, [10, 25])).toBe(10);
  });

  it("leaves buckets before the first seam unclamped", () => {
    expect(rollingWindowStart(4, [10, 25])).toBe(0);
  });

  it("leaves a non-Conflux chart unclamped throughout", () => {
    expect(rollingWindowStart(0, [])).toBe(0);
    expect(rollingWindowStart(99, [])).toBe(0);
  });
});

describe("confluxBossLines", () => {
  it("draws a solid rule, against the dashed area seams", () => {
    const [boss] = confluxBossLines([30_000], DPS_MS, 40);
    const [seam] = confluxSeamLines([30_000], DPS_MS, 40);

    expect(boss.props.strokeDasharray).toBeUndefined();
    expect(seam.props.strokeDasharray).toBe("4 4");
    expect(boss.props.x).toBe(seam.props.x);
    expect(boss.props.yAxisId).toBe("left");
  });
});

describe("SBA lockdown and Break overlays", () => {
  it("shades an SBA lockdown in yellow, against the left axis", () => {
    const [area] = sbaLockdownAreas([[19_321, 47_423]], DPS_MS, 40);

    expect(area.props.fill).toBe("#ffe066");
    expect(area.props.yAxisId).toBe("left");
  });

  it("shades Break far more faintly than the SBA window", () => {
    const [sba] = sbaLockdownAreas([[0, 30_000]], DPS_MS, 40);
    const [brk] = breakAreas([[0, 30_000]], DPS_MS, 40);

    expect(brk.props.fill).toBe("#ced4da");
    expect(brk.props.fillOpacity).toBeLessThan(sba.props.fillOpacity);
  });

  it("renders nothing for a log with neither captured", () => {
    expect(sbaLockdownAreas([], DPS_MS, 40)).toEqual([]);
    expect(breakAreas([], DPS_MS, 40)).toEqual([]);
  });
});
