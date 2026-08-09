import { ActionIcon, Box, Divider, Group, Modal, Paper, SimpleGrid, Stack, Tabs, Text, Tooltip } from "@mantine/core";
import { Question } from "@phosphor-icons/react";
import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import { boardGemCounts, masterTraitStyleBlocks, styleBoardName } from "@/models/skillboard";
import { type PlayerData } from "@/types";

const STYLE_RANK_LABELS: Record<string, string> = {
  rank1: "Style Rank 1",
  rank2: "Style Rank 2",
  rank3: "Style Rank 3",
  EX: "Style Rank EX",
};

export type MasterTraitsModalProps = {
  opened: boolean;
  onClose: () => void;
  player: PlayerData | null;
};

export const MasterTraitsModal = ({ opened, onClose, player }: MasterTraitsModalProps) => {
  const { t } = useTranslation();

  return (
    <Modal opened={opened} onClose={onClose} title={t("ui.player-master-traits", "Master Traits")} size="xl">
      {player &&
        (() => {
          const flags = player.masterTraitFlags || [];
          const blocks = masterTraitStyleBlocks(flags);
          if (blocks.length === 0) return null;

          const cardName = (block: (typeof blocks)[number]) => styleBoardName(block.cardHash, block.board);

          const styleLabel = (block: (typeof blocks)[number]) => {
            const full = cardName(block);
            const idx = full.indexOf(":");
            return idx > 0 ? full.slice(0, idx).trim() : full;
          };

          return (
            <Tabs defaultValue={blocks[0].board} variant="outline">
              <Tabs.List>
                {blocks.map((block) => (
                  <Tabs.Tab key={block.board} value={block.board}>
                    {styleLabel(block)} ({boardGemCounts(flags, block.board).join("/")})
                  </Tabs.Tab>
                ))}
              </Tabs.List>
              {blocks.map((block) => (
                <Tabs.Panel key={block.board} value={block.board} pt="sm">
                  {block.perks.length > 0 && (
                    <Group gap={6} align="center" mb="sm">
                      <Text fw={700} size="sm">
                        {cardName(block)}
                      </Text>
                      <Tooltip
                        multiline
                        w={340}
                        color="dark"
                        label={
                          <Stack gap={4}>
                            <Text size="xs" fw={700} c="white">
                              {t("ui.master-traits-perk-bonuses", "Style Rank Perk Bonuses")}
                            </Text>
                            {block.perks.map((perk, perkIndex) => (
                              <Fragment key={perk.hash}>
                                {perkIndex > 0 && <Divider color="gray.6" style={{ opacity: 0.4 }} />}
                                <Text
                                  size="xs"
                                  c="white"
                                  style={{ whiteSpace: "pre-wrap", opacity: perk.on ? 1 : 0.4 }}
                                >
                                  {perk.text}
                                </Text>
                              </Fragment>
                            ))}
                          </Stack>
                        }
                      >
                        <ActionIcon
                          variant="subtle"
                          color="gray"
                          size="sm"
                          aria-label={t("ui.master-traits-perks", "Style Rank Perks")}
                        >
                          <Question size={16} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                  )}
                  {block.groups.map((rankGroup) => (
                    <Box key={rankGroup.group} mb="sm">
                      <Text fw={600} size="xs" c="dimmed" mb={4}>
                        {STYLE_RANK_LABELS[rankGroup.group] ?? rankGroup.group}
                      </Text>
                      <SimpleGrid cols={2} spacing="xs" verticalSpacing="xs">
                        {rankGroup.nodes.map((node) => (
                          <Paper
                            key={node.hash}
                            withBorder
                            radius={0}
                            p="xs"
                            style={{
                              display: "flex",
                              alignItems: "center",
                              minHeight: 48,
                              opacity: node.on ? 1 : 0.35,
                            }}
                          >
                            <Text
                              size="xs"
                              fw={300}
                              c={node.on ? undefined : "dimmed"}
                              style={{ whiteSpace: "pre-wrap" }}
                            >
                              {node.text}
                            </Text>
                          </Paper>
                        ))}
                      </SimpleGrid>
                    </Box>
                  ))}
                </Tabs.Panel>
              ))}
            </Tabs>
          );
        })()}
    </Modal>
  );
};
