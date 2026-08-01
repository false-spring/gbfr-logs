import { LineChart } from "@mantine/charts";
import { Box, Text } from "@mantine/core";

import { ChartTooltip } from "@/components/ChartTooltip";
import { useTranslation } from "react-i18next";

import { CHART_Y_AXIS_WIDTH } from "@/components/chartCommon";
import { type StatusStackSample } from "@/types";
import { millisecondsToElapsedPreciseFormat } from "@/utils/format";
import { statusPointsForChart } from "@/utils/status";

export const STACK_CHART_HEIGHT_RATIO = 1 / 5;

type Props = {
  samples: StatusStackSample[];
  extentMs: number;
  parentHeight: number;
};

export const StackDepthChart = ({ samples, extentMs, parentHeight }: Props) => {
  const { t } = useTranslation();

  const data = statusPointsForChart(samples, extentMs).map((point) => ({
    timestamp: point.t,
    stacks: point.value,
  }));

  if (data.length === 0) return null;

  const peak = samples.reduce((deepest, [, depth]) => Math.max(deepest, depth), 0);

  return (
    <Box>
      <Text size="xs" c="dimmed" mb={4}>
        {t("ui.logs.status-stack-count", "Stacks")}
      </Text>
      <LineChart
        h={Math.round(parentHeight * STACK_CHART_HEIGHT_RATIO)}
        data={data}
        dataKey="timestamp"
        withDots={false}
        curveType="stepAfter"
        yAxisProps={{ domain: [0, peak], allowDecimals: false, width: CHART_Y_AXIS_WIDTH }}
        xAxisProps={{ type: "number", domain: [0, extentMs], hide: true }}
        tooltipProps={{
          content: ({ label, payload }) => (
            <ChartTooltip label={millisecondsToElapsedPreciseFormat(Number(label))} payload={payload} />
          ),
        }}
        connectNulls={false}
        series={[{ name: "stacks", color: "grape.5", label: t("ui.logs.status-stack-count", "Stacks") }]}
      />
    </Box>
  );
};
