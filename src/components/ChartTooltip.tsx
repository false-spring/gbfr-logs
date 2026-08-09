import { Paper, Text } from "@mantine/core";
import { t } from "i18next";

export interface ChartTooltipProps {
  label: string;
  payload: Record<string, any>[] | undefined; // eslint-disable-line
  partyLabel?: string;
}

export const ChartTooltip = ({ label, payload, partyLabel }: ChartTooltipProps) => {
  if (!payload) return null;

  return (
    <Paper px="md" py="sm" withBorder shadow="md" radius="md">
      <Text fw={500} mb={5}>
        {label}
      </Text>
      {payload.map(
        (
          item: any // eslint-disable-line
        ) => (
          <Text key={item.name} fz="sm">
            <Text component="span" c={item.color}>
              {item.name === "party" ? partyLabel ?? t("ui.logs.damage-per-second") : item.name}
            </Text>
            : {new Intl.NumberFormat("en-US").format(item.value)}
          </Text>
        )
      )}
    </Paper>
  );
};
