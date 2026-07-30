import { ActionIcon, Box, Button, Checkbox, Flex, Stack, Table, Text, Tooltip } from "@mantine/core";
import { Calculator, ClipboardText } from "@phosphor-icons/react";
import { t } from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import SummonEquipBonuses from "@/assets/summon-equip-bonus";
import TraitOrder from "@/assets/trait-order";
import {
  masterTraitStyleBlocks,
  masteryLevel,
  styleBoardName,
  styleBoardSummaries,
  styleRankDiamonds,
} from "@/models/skillboard";
import { type OverMasteryLine, type Overmastery, type PlayerData, type SummonSlot } from "@/types";
import { EMPTY_ID } from "@/utils/constants";
import {
  categoryTint,
  inferWrightstoneFamily,
  isFlatOvermasteryValue,
  isFlatSummonBonus,
  overMasteryDisplayValue,
  summonEquipBonusValue,
} from "@/utils/equipment";
import { toHashString } from "@/utils/format";
import {
  translateItemId,
  translateOvermasteryId,
  translateSigilId,
  translateSummonBonusId,
  translateSummonId,
  translateTraitId,
} from "@/utils/i18n";

const isEmptySummon = (summon: SummonSlot | undefined): boolean => {
  return !summon || summon.id === EMPTY_ID || summon.traitId === EMPTY_ID;
};

type SummonParts = { name: string | null; trait: string; bonus: string | null };

const summonParts = (summon: SummonSlot | undefined): SummonParts | null => {
  if (!summon) return null;

  const traitLevel = summon.traitLevel > 0 ? summon.traitLevel : 0;
  const trait = `${translateTraitId(summon.traitId)} (T.Lvl ${traitLevel})`;
  const name = translateSummonId(summon.id) || null;

  const bonusName = translateSummonBonusId(summon.equipBonusId);
  const bonusEntry = SummonEquipBonuses[toHashString(summon.equipBonusId)];
  const bonusValue = summonEquipBonusValue(bonusEntry?.ladder, bonusEntry?.display_mult ?? 1, summon.equipBonusLevel);
  const bonus =
    bonusName && bonusValue !== null
      ? `${bonusName} +${bonusValue}${isFlatSummonBonus(summon.equipBonusId) ? "" : "%"}`
      : null;

  return { name, trait, bonus };
};

const SummonLines = ({ summon }: { summon: SummonSlot | undefined }) => {
  const parts = summonParts(summon);
  if (!parts) return null;

  return (
    <Box mb={4}>
      {parts.name && (
        <Text size="xs" fw={700}>
          {parts.name}
        </Text>
      )}
      <Text size="xs" fs="italic" fw={300}>
        · {parts.trait}
      </Text>
      {parts.bonus && (
        <Text size="xs" fs="italic" fw={300}>
          · {parts.bonus}
        </Text>
      )}
    </Box>
  );
};

const formatSkill = (id: number | undefined): string => {
  if (!id || id === EMPTY_ID) return "-";
  return t(`skills:${toHashString(id)}.text`, { defaultValue: `#${toHashString(id)}` });
};

const isEmptyOverMasteryLine = (line: OverMasteryLine | undefined): boolean => {
  return !line || line.id === EMPTY_ID || line.id === 0 || line.rank === 0;
};

const formatOverMasteryLine = (line: OverMasteryLine | undefined): string => {
  if (!line) return "";

  const translation = translateOvermasteryId(line.id);
  const value = overMasteryDisplayValue(line.id, line.value);
  const suffix = isFlatOvermasteryValue(line.id) ? "" : "%";

  return `${translation} +${value}${suffix}`;
};

const sigilTypeTint = (firstTraitId: number): string | undefined => {
  const category = TraitOrder[toHashString(firstTraitId)]?.category;
  return category === undefined ? undefined : categoryTint(category);
};

// An opaque base layer hides the row striping so the tint reads the same on
// odd and even rows.
const sigilCellBackground = (showColors: boolean, tint: string | undefined): string | undefined => {
  if (!showColors) return undefined;
  const base = "var(--mantine-color-body)";
  return tint ? `linear-gradient(${tint}, ${tint}), ${base}` : base;
};

const formatOvermastery = (overmastery: Overmastery | undefined): string => {
  if (!overmastery) return "";

  const translation = translateOvermasteryId(overmastery.id);
  const value = (overmastery.value ?? 0).toFixed(0);

  if (isFlatOvermasteryValue(overmastery.id)) {
    return `${translation}: +${value}`;
  } else {
    return `${translation}: +${value}%`;
  }
};

// Returns a string of stars based on the star level.
// ★★★☆☆☆ (3 stars)
// ★★★★★★ (6 stars)
const createWeaponStars = (starLevel: number): string => {
  return "★".repeat(starLevel) + "☆".repeat(6 - starLevel);
};

// The damage calculator it linked to no longer works.
const SHOW_DAMAGE_CALCULATOR = false;

function Placeholder({ empty, children }: { empty: boolean; children?: React.ReactNode }) {
  return empty ? (
    <Text size="xs" fw={300}>
      ---
    </Text>
  ) : (
    children
  );
}

function TraitLine({ id, level }: { id: number | undefined; level: number | undefined }) {
  return (
    <Placeholder empty={!id || level == 0}>
      <Text size="xs" fs="italic" fw={300}>
        - {translateTraitId(id || EMPTY_ID)} (Lvl. {level})
      </Text>
    </Placeholder>
  );
}

export type EquipmentTabProps = {
  playerData: PlayerData[];
  playerNames: string[];
  onCopyCharacterData: (player: PlayerData) => void;
  onOpenDamageCalculator: (player: PlayerData) => void;
  onShowMasterTraitsDetails: (player: PlayerData) => void;
};

export const EquipmentTab = ({
  playerData,
  playerNames,
  onCopyCharacterData,
  onOpenDamageCalculator,
  onShowMasterTraitsDetails,
}: EquipmentTabProps) => {
  const { t } = useTranslation();
  const [showSigilColors, setShowSigilColors] = useState(false);

  // The 13th sigil slot never shows in-game; render its row only when occupied.
  const SIGIL_SLOTS = 13;
  const sigilRowCount = playerData.some(
    (player) => player.sigils[SIGIL_SLOTS - 1] && player.sigils[SIGIL_SLOTS - 1].sigilId !== EMPTY_ID
  )
    ? SIGIL_SLOTS
    : SIGIL_SLOTS - 1;

  return (
    <Stack mt="20" gap="xs">
      <Table striped layout="fixed">
        <Table.Tbody>
          <Table.Tr>
            {playerData.map((player, i) => {
              return (
                <Table.Td key={player.actorIndex} flex={1}>
                  <Flex direction="row" wrap="nowrap" align="center">
                    <Text fw={700} size="xl" mr="5">
                      {playerNames[i]}
                    </Text>
                    <Tooltip label={t("ui.copy-character-data-to-clipboard")} color="dark">
                      <ActionIcon
                        aria-label="Clipboard"
                        variant="filled"
                        color="light"
                        onClick={() => onCopyCharacterData(player)}
                      >
                        <ClipboardText size={16} />
                      </ActionIcon>
                    </Tooltip>
                    {SHOW_DAMAGE_CALCULATOR && (
                      <Tooltip label={t("ui.open-damage-calculator")} color="dark">
                        <ActionIcon
                          aria-label="Open build"
                          variant="filled"
                          color="light"
                          onClick={() => onOpenDamageCalculator(player)}
                        >
                          <Calculator size={16} />
                        </ActionIcon>
                      </Tooltip>
                    )}
                  </Flex>
                </Table.Td>
              );
            })}
          </Table.Tr>
          <Table.Tr>
            {playerData.map((player) => {
              return (
                <Table.Td key={player.actorIndex}>
                  <Text size="xs" fw={700}>
                    {t("ui.player-stats")}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.level")}: {player.playerStats?.level || 1}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.total-hp")}: {player.playerStats?.totalHp || "---"}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.total-attack")}: {player.playerStats?.totalAttack || "---"}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.critical-rate")}:{" "}
                    {player.playerStats?.criticalRate ? `${player.playerStats.criticalRate.toFixed(0)}%` : "---"}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.stun-power")}:{" "}
                    {player.playerStats?.stunPower ? (player.playerStats.stunPower * 10).toFixed(0) : "---"}
                  </Text>
                  <Text size="xs" fs="italic" fw={300}>
                    {t("ui.stats.total-power")}: {player.playerStats?.totalPower || "---"}
                  </Text>
                  {/* DMG-cap channels are captured but not shown: no known valid
                      transform from the stored values to a display %. */}
                </Table.Td>
              );
            })}
          </Table.Tr>
          {playerData.some((player) => (player.skillLoadout?.length ?? 0) > 0) && (
            <Table.Tr>
              {playerData.map((player) => {
                const skills = player.skillLoadout || [];

                return (
                  <Table.Td key={player.actorIndex}>
                    <Text size="xs" fw={700}>
                      {t("ui.player-skills", "Skills")}
                    </Text>
                    {Array.from(Array(4).keys()).map((skillIndex) => (
                      <Text key={skillIndex} size="xs" fs="italic" fw={300}>
                        {formatSkill(skills[skillIndex])}
                      </Text>
                    ))}
                  </Table.Td>
                );
              })}
            </Table.Tr>
          )}
          {playerData.some((player) => (player.summonInfo?.summons.length ?? 0) > 0) && (
            <Table.Tr>
              {playerData.map((player) => {
                const summons = player.summonInfo?.summons || [];

                return (
                  <Table.Td key={player.actorIndex}>
                    <Text size="xs" fw={700}>
                      {t("ui.player-summons", "Summons")}
                    </Text>
                    {Array.from(Array(4).keys()).map((summonIndex) => {
                      const summon = summons[summonIndex];

                      return (
                        <Placeholder key={summonIndex} empty={isEmptySummon(summon)}>
                          <SummonLines summon={summon} />
                        </Placeholder>
                      );
                    })}
                  </Table.Td>
                );
              })}
            </Table.Tr>
          )}
          {playerData.some((player) => (player.overMastery ?? []).some((line) => !isEmptyOverMasteryLine(line))) && (
            <Table.Tr>
              {playerData.map((player) => {
                const overMastery = player.overMastery || [];

                return (
                  <Table.Td key={player.actorIndex}>
                    <Text size="xs" fw={700}>
                      {t("ui.player-overmasteries")}
                    </Text>
                    {Array.from(Array(4).keys()).map((lineIndex) => {
                      const line = overMastery[lineIndex];

                      return (
                        <Placeholder key={lineIndex} empty={isEmptyOverMasteryLine(line)}>
                          <Text size="xs" fs="italic" fw={300}>
                            {formatOverMasteryLine(line)}
                          </Text>
                        </Placeholder>
                      );
                    })}
                  </Table.Td>
                );
              })}
            </Table.Tr>
          )}
          {playerData.some((player) => (player.masterTraitFlags?.length ?? 0) > 0) && (
            <Table.Tr>
              {playerData.map((player) => {
                const flags = player.masterTraitFlags || [];
                const boards = styleBoardSummaries(flags);
                const mLvl = masteryLevel(flags);
                const hasSkillboard = boards.some((entry) => entry.ranksTotal > 0);

                return (
                  <Table.Td key={player.actorIndex}>
                    <Text size="xs" fw={700}>
                      {t("ui.player-master-traits", "Master Traits")} (Lv. {mLvl})
                    </Text>
                    {!hasSkillboard ? (
                      <Placeholder empty />
                    ) : (
                      boards.map(({ board, cardHash, ranksOn, ranksTotal, gems }) => (
                        <Text key={board} size="xs" fs="italic" fw={300}>
                          {[
                            styleBoardName(cardHash, board),
                            styleRankDiamonds(ranksOn, ranksTotal),
                            `(${gems.join("/")})`,
                          ]
                            .filter((part) => part !== "")
                            .join(" ")}
                        </Text>
                      ))
                    )}
                    {masterTraitStyleBlocks(flags).length > 0 && (
                      <Button
                        variant="subtle"
                        size="compact-xs"
                        mt={4}
                        onClick={() => onShowMasterTraitsDetails(player)}
                      >
                        {t("ui.player-master-traits-details", "Show Details")}
                      </Button>
                    )}
                  </Table.Td>
                );
              })}
            </Table.Tr>
          )}
          {playerData.some((player) => (player.overmasteryInfo?.overmasteries.length ?? 0) > 0) && (
            <Table.Tr>
              {playerData.map((player) => {
                const overmasteries = player.overmasteryInfo?.overmasteries || [];

                return (
                  <Table.Td key={player.actorIndex}>
                    <Text size="xs" fw={700}>
                      {t("ui.player-overmasteries")}
                    </Text>
                    {Array.from(Array(4).keys()).map((overmasteryIndex) => {
                      const overmastery = overmasteries[overmasteryIndex];

                      return (
                        <Placeholder key={overmasteryIndex} empty={!overmastery || overmastery.value === 0}>
                          <Text size="xs" fs="italic" fw={300}>
                            {formatOvermastery(overmastery)}
                          </Text>
                        </Placeholder>
                      );
                    })}
                  </Table.Td>
                );
              })}
            </Table.Tr>
          )}
          <Table.Tr>
            {playerData.map((player) => {
              const weaponId = player.weaponInfo?.weaponId || 0;
              const hasWeapon = weaponId !== 0 && weaponId !== EMPTY_ID;

              return (
                <Table.Td key={player.actorIndex}>
                  <Text size="xs" fw={700}>
                    {t("ui.weapon")}
                  </Text>
                  {(player.weaponInfo?.starLevel || 0) > 0 && (
                    <Text size="xs" fs="italic" fw={300}>
                      {createWeaponStars(player.weaponInfo?.starLevel || 0)}
                    </Text>
                  )}
                  <Placeholder empty={!hasWeapon}>
                    <Text size="xs" fs="italic" fw={300}>
                      {t([`weapons:${toHashString(weaponId)}.text`, "unknown"])}
                      {(player.weaponInfo?.plusMarks || 0) > 0 && ` +${player.weaponInfo?.plusMarks}`}
                    </Text>
                  </Placeholder>
                  {(player.weaponInfo?.awakeningLevel || 0) > 0 && (
                    <Text size="xs" fs="italic" fw={300}>
                      Awakening {player.weaponInfo?.awakeningLevel}/10
                    </Text>
                  )}
                  <Placeholder empty={(player.weaponInfo?.weaponLevel || 0) === 0}>
                    <Text size="xs" fs="italic" fw={300}>
                      Lvl {player.weaponInfo?.weaponLevel || 0}
                      {(player.weaponInfo?.transcendenceLevel || 0) > 0 &&
                        ` · Trans. ${player.weaponInfo?.transcendenceLevel}`}
                      {(player.weaponInfo?.awakeningLevelEr || 0) > 0 &&
                        ` · Awake ${player.weaponInfo?.awakeningLevelEr}`}
                      {(player.weaponInfo?.weaponAttack || 0) > 0 && ` / ATK ${player.weaponInfo?.weaponAttack}`}
                      {(player.weaponInfo?.weaponHp || 0) > 0 && ` / HP ${player.weaponInfo?.weaponHp}`}
                    </Text>
                  </Placeholder>
                  <TraitLine id={player.weaponInfo?.trait1Id} level={player.weaponInfo?.trait1Level} />
                  <TraitLine id={player.weaponInfo?.trait2Id} level={player.weaponInfo?.trait2Level} />
                  <TraitLine id={player.weaponInfo?.trait3Id} level={player.weaponInfo?.trait3Level} />
                  <TraitLine id={player.weaponInfo?.trait4Id} level={player.weaponInfo?.trait4Level} />
                  <TraitLine id={player.weaponInfo?.trait5Id} level={player.weaponInfo?.trait5Level} />
                  {(() => {
                    // The wrightstone item id is not replicated in ER.
                    const wid = player.weaponInfo?.wrightstoneId || 0;
                    const hasWrightstoneId = wid !== 0 && wid !== EMPTY_ID;
                    const wtIds = [
                      player.weaponInfo?.wrightstoneTrait1Id,
                      player.weaponInfo?.wrightstoneTrait2Id,
                      player.weaponInfo?.wrightstoneTrait3Id,
                    ];
                    const hasWrightstoneTraits = wtIds.some((id) => id && id !== EMPTY_ID);

                    if (hasWrightstoneId) {
                      return (
                        <Text size="xs" fw={700}>
                          {translateItemId(wid)}
                        </Text>
                      );
                    }
                    if (hasWrightstoneTraits) {
                      const inferredFamily = inferWrightstoneFamily([
                        {
                          id: player.weaponInfo?.wrightstoneTrait1Id,
                          level: player.weaponInfo?.wrightstoneTrait1Level,
                        },
                        {
                          id: player.weaponInfo?.wrightstoneTrait2Id,
                          level: player.weaponInfo?.wrightstoneTrait2Level,
                        },
                        {
                          id: player.weaponInfo?.wrightstoneTrait3Id,
                          level: player.weaponInfo?.wrightstoneTrait3Level,
                        },
                      ]);
                      if (inferredFamily) {
                        return (
                          <Text size="xs" fw={700}>
                            {inferredFamily}
                          </Text>
                        );
                      }
                      return (
                        <Text size="xs" fw={700}>
                          {t("ui.wrightstone-id-unavailable", "Wrightstone (id n/a online)")}
                        </Text>
                      );
                    }
                    return null;
                  })()}
                  <TraitLine
                    id={player.weaponInfo?.wrightstoneTrait1Id}
                    level={player.weaponInfo?.wrightstoneTrait1Level}
                  />
                  <TraitLine
                    id={player.weaponInfo?.wrightstoneTrait2Id}
                    level={player.weaponInfo?.wrightstoneTrait2Level}
                  />
                  <TraitLine
                    id={player.weaponInfo?.wrightstoneTrait3Id}
                    level={player.weaponInfo?.wrightstoneTrait3Level}
                  />
                </Table.Td>
              );
            })}
          </Table.Tr>
          {Array.from(Array(sigilRowCount).keys()).map((sigilIndex) => (
            <Table.Tr key={sigilIndex}>
              {playerData.map((player) => {
                const sigil = player.sigils[sigilIndex];

                if (!sigil || sigil.sigilId === EMPTY_ID) {
                  return (
                    <Table.Td
                      key={player.actorIndex}
                      style={{ background: sigilCellBackground(showSigilColors, undefined) }}
                    >
                      <Placeholder empty />
                    </Table.Td>
                  );
                }

                return (
                  <Table.Td
                    key={player.actorIndex}
                    style={{
                      background: sigilCellBackground(showSigilColors, sigilTypeTint(sigil.firstTraitId)),
                    }}
                  >
                    <Text size="xs" fw={700}>
                      {translateSigilId(sigil.sigilId)} (Lvl. {sigil.sigilLevel})
                    </Text>
                    <Text size="xs" fs="italic" fw={300}>
                      {translateTraitId(sigil.firstTraitId)} (Lvl. {sigil.firstTraitLevel})
                      {sigil.secondTraitId !== EMPTY_ID &&
                        ` / ${translateTraitId(sigil.secondTraitId)} (Lvl. ${sigil.secondTraitLevel})`}
                    </Text>
                  </Table.Td>
                );
              })}
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
      <Checkbox
        size="xs"
        label={t("ui.logs.show-sigil-type-colors", "Show Sigil Type Colors")}
        checked={showSigilColors}
        onChange={(event) => setShowSigilColors(event.currentTarget.checked)}
      />
    </Stack>
  );
};
