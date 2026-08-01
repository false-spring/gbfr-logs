import { LineChart } from "@mantine/charts";
import { Box, Text } from "@mantine/core";

import { ChartTooltip } from "@/components/ChartTooltip";
import { useTranslation } from "react-i18next";

import { STACK_CHART_HEIGHT_RATIO } from "@/components/StackDepthChart";
import { CHART_Y_AXIS_WIDTH } from "@/components/chartCommon";
import { type StatusValueSample } from "@/types";
import { millisecondsToElapsedPreciseFormat } from "@/utils/format";
import { formatStatusValue, statusPointsForChart } from "@/utils/status";

type Props = {
  samples: StatusValueSample[];
  extentMs: number;
  parentHeight: number;
  isFraction: boolean | undefined;
};

// The raw additive sum, no ceiling: the engine's clamp on the combined
// multiplier is attacker-side, not a cap on the target's debuff stack.
export const StatusMagnitudeChart = ({ samples, extentMs, parentHeight, isFraction }: Props) => {
  const { t } = useTranslation();

  const data = statusPointsForChart(samples, extentMs).map((point) => ({
    timestamp: point.t,
    value: point.value,
  }));

  if (data.length === 0) return null;

  const peak = samples.reduce((highest, [, v]) => Math.max(highest, v), 0);

  return (
    <Box>
      <Text size="xs" c="dimmed" mb={4}>
        {t("ui.logs.status-magnitude", "Magnitude")}
      </Text>
      <LineChart
        h={Math.round(parentHeight * STACK_CHART_HEIGHT_RATIO)}
        data={data}
        dataKey="timestamp"
        withDots={false}
        curveType="stepAfter"
        yAxisProps={{
          domain: [0, peak],
          width: CHART_Y_AXIS_WIDTH,
          tickFormatter: (value: number) => formatStatusValue(value, isFraction),
        }}
        valueFormatter={(value) => formatStatusValue(value, isFraction)}
        xAxisProps={{ type: "number", domain: [0, extentMs], hide: true }}
        tooltipProps={{
          content: ({ label, payload }) => (
            <ChartTooltip label={millisecondsToElapsedPreciseFormat(Number(label))} payload={payload} />
          ),
        }}
        connectNulls={false}
        series={[{ name: "value", color: "teal.5", label: t("ui.logs.status-magnitude", "Magnitude") }]}
      />
    </Box>
  );
};
