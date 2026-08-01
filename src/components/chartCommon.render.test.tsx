import { render } from "@testing-library/react";
import { Line, LineChart, XAxis, YAxis } from "recharts";
import { describe, expect, it } from "vitest";

import { statusAreas } from "@/components/chartCommon";
import { type LinkTimeWindow } from "@/types";
import { millisecondsToElapsedFormat } from "@/utils/format";

const DPS_MS = 3_000;
const DPS_BUCKETS = 41;
const DPS_SPAN_MS = (DPS_BUCKETS - 1) * DPS_MS;

const plotRects = (windows: LinkTimeWindow[]) => {
  const data = Array.from({ length: DPS_BUCKETS }, (_, i) => ({
    timestamp: millisecondsToElapsedFormat(i * DPS_MS),
    party: i,
  }));

  const { container } = render(
    <LineChart width={700} height={300} data={data}>
      <XAxis dataKey="timestamp" />
      <YAxis yAxisId="left" />
      <Line yAxisId="left" dataKey="party" isAnimationActive={false} />
      {statusAreas(windows, DPS_MS, DPS_BUCKETS)}
    </LineChart>
  );

  return [...container.querySelectorAll(".recharts-reference-area rect")].map((rect) => ({
    x: parseFloat(rect.getAttribute("x") ?? "NaN"),
    width: parseFloat(rect.getAttribute("width") ?? "NaN"),
    height: parseFloat(rect.getAttribute("height") ?? "NaN"),
    fill: rect.getAttribute("fill"),
    fillOpacity: rect.getAttribute("fill-opacity"),
  }));
};

describe("the status overlay against a real chart", () => {
  it("paints a sub-bucket window a fraction of a bucket wide", () => {
    const [plot] = plotRects([[0, DPS_SPAN_MS]]);
    const [narrow] = plotRects([[3_100, 3_500]]);

    expect(plot.width).toBeGreaterThan(0);
    expect(plot.height).toBeGreaterThan(0);

    expect(narrow.x).toBeCloseTo(plot.x + (3_100 / DPS_SPAN_MS) * plot.width, 6);
    expect(narrow.width).toBeCloseTo((400 / DPS_SPAN_MS) * plot.width, 6);
    expect(narrow.width).toBeLessThan(plot.width / (DPS_BUCKETS - 1));
    expect(narrow.height).toBe(plot.height);
  });

  it("draws one rect per span, in order", () => {
    const rects = plotRects([
      [30_000, 36_000],
      [3_000, 9_000],
    ]);

    expect(rects).toHaveLength(2);
    expect(rects[0].x).toBeLessThan(rects[1].x);
  });

  it("takes its fill from the area, which is what recharts forwards to a shape", () => {
    const [narrow] = plotRects([[3_100, 3_500]]);

    expect(narrow.fill).toBe("#cc5de8");
    expect(narrow.fillOpacity).toBe("0.22");
  });

  it("draws nothing when no status is selected", () => {
    expect(plotRects([])).toEqual([]);
  });
});
