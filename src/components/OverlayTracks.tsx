import { Box, Stack, Text } from "@mantine/core";

import { overlayTrackSegments } from "@/components/chartCommon";
import { type LinkTimeWindow } from "@/types";

export type OverlayTrack = {
  label: string;
  color: string;
  windows: LinkTimeWindow[];
};

export type OverlayTracksProps = {
  tracks: OverlayTrack[];
  intervalMs: number;
  bucketCount: number;
};

// The plot area starts after the 60px y axis plus the chart's 5px left
// margin, and ends 5px short of the right edge.
const PLOT_LEFT_OFFSET = 65;
const PLOT_RIGHT_OFFSET = 5;

const TRACK_HEIGHT = 8;

export const OverlayTracks = ({ tracks, intervalMs, bucketCount }: OverlayTracksProps) => (
  <Stack gap={2} w="100%">
    {tracks.map((track) => (
      <Box key={track.label} display="flex" style={{ alignItems: "center" }}>
        <Text size="xs" c="dimmed" w={PLOT_LEFT_OFFSET} truncate>
          {track.label}
        </Text>
        <Box
          pos="relative"
          h={TRACK_HEIGHT}
          style={{
            flex: 1,
            marginRight: PLOT_RIGHT_OFFSET,
            borderRadius: 2,
            backgroundColor: "var(--mantine-color-default-border)",
          }}
        >
          {overlayTrackSegments(track.windows, intervalMs, bucketCount).map((segment) => (
            <Box
              key={segment.left}
              pos="absolute"
              h="100%"
              style={{
                left: `${segment.left}%`,
                width: `${segment.width}%`,
                backgroundColor: track.color,
                borderRadius: 2,
              }}
            />
          ))}
        </Box>
      </Box>
    ))}
  </Stack>
);
