import { Fragment, useState } from "react";
import { CharacterType, ComputedPlayerState, ComputedSkillState, PlayerData } from "../types";
import { humanizeNumbers, getSkillName, translatedPlayerName } from "../utils";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { useMeterSettingsStore } from "../Store";
import { useShallow } from "zustand/react/shallow";

type Props = {
  player: ComputedPlayerState;
  color: string;
};

const SkillRow = ({
  characterType,
  skill,
  color,
}: {
  characterType: CharacterType;
  skill: ComputedSkillState;
  color: string;
}) => {
  const [totalDamage, totalDamageUnit] = humanizeNumbers(skill.totalDamage);
  const [minDmg, minDmgUnit] = humanizeNumbers(skill.minDamage || 0);
  const [maxDmg, maxDmgUnit] = humanizeNumbers(skill.maxDamage || 0);
  const [averageDmg, averageDmgUnit] = humanizeNumbers(skill.hits === 0 ? 0 : skill.totalDamage / skill.hits);

  return (
    <tr className="skill-row">
      <td className="text-left row-data">{getSkillName(characterType, skill)}</td>
      <td className="text-center row-data">{skill.hits}</td>
      <td className="text-center row-data">
        {totalDamage}
        <span className="unit font-sm">{totalDamageUnit}</span>
      </td>
      <td className="text-center row-data">
        {skill.minDamage && minDmg}
        <span className="unit font-sm">{minDmgUnit}</span>
      </td>
      <td className="text-center row-data">
        {skill.maxDamage && maxDmg}
        <span className="unit font-sm">{maxDmgUnit}</span>
      </td>
      <td className="text-center row-data">
        {averageDmg}
        <span className="unit font-sm">{averageDmgUnit}</span>
      </td>
      <td className="text-center row-data">
        {skill.percentage.toFixed(0)}
        <span className="unit font-sm">%</span>
      </td>
      <div className="damage-bar" style={{ backgroundColor: color, width: `${skill.percentage}%` }} />
    </tr>
  );
};

const SkillBreakdown = ({ player, color }: Props) => {
  const totalDamage = player.skillBreakdown.reduce((acc, skill) => acc + skill.totalDamage, 0);
  const computedSkills = player.skillBreakdown.map((skill) => {
    return {
      percentage: (skill.totalDamage / totalDamage) * 100,
      ...skill,
    };
  });

  computedSkills.sort((a, b) => b.totalDamage - a.totalDamage);

  return (
    <tr className="skill-table">
      <td colSpan={100}>
        <table className="table w-full">
          <thead className="header transparent-bg">
            <tr>
              <th className="header-name">Skill</th>
              <th className="header-column text-center">Hits</th>
              <th className="header-column text-center">Total</th>
              <th className="header-column text-center">Min</th>
              <th className="header-column text-center">Max</th>
              <th className="header-column text-center">Avg</th>
              <th className="header-column text-center">%</th>
            </tr>
          </thead>
          <tbody className="transparent-bg">
            {computedSkills.map((skill) => (
              <SkillRow
                key={`${skill.childCharacterType}-${getSkillName(player.characterType, skill)}`}
                characterType={player.characterType}
                skill={skill}
                color={color}
              />
            ))}
          </tbody>
        </table>
      </td>
    </tr>
  );
};

export const PlayerRow = ({
  player,
  partyData,
}: {
  player: ComputedPlayerState;
  partyData: Array<PlayerData | null>;
}) => {
  const { color_1, color_2, color_3, color_4, show_display_names } = useMeterSettingsStore(
    useShallow((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
      show_display_names: state.show_display_names,
    }))
  );

  const playerColors = [color_1, color_2, color_3, color_4, "#9BCF53", "#380E7F", "#416D19", "#2C568D"];
  const partySlotIndex = partyData.findIndex((partyMember) => partyMember?.actorIndex === player.index);
  const color = partySlotIndex !== -1 ? playerColors[partySlotIndex] : playerColors[player.partyIndex];

  const [isOpen, setIsOpen] = useState(false);

  const [totalDamage, totalDamageUnit] = humanizeNumbers(player.totalDamage);
  const [dps, dpsUnit] = humanizeNumbers(player.dps);

  return (
    <Fragment>
      <tr className={`player-row ${isOpen ? "transparent-bg" : ""}`} onClick={() => setIsOpen(!isOpen)}>
        <td className="text-left row-data">
          {translatedPlayerName(partySlotIndex, partyData[partySlotIndex], player, show_display_names)}
        </td>
        <td className="text-center row-data">
          {totalDamage}
          <span className="unit font-sm">{totalDamageUnit}</span>
        </td>
        <td className="text-center row-data">
          {dps}
          <span className="unit font-sm">{dpsUnit}</span>
        </td>
        <td className="text-center row-data">
          {player.percentage?.toFixed(0)}
          <span className="unit font-sm">%</span>
        </td>
        <td className="text-center row-button">{isOpen ? <CaretUp size={16} /> : <CaretDown size={16} />}</td>
        <div className="damage-bar" style={{ backgroundColor: color, width: `${player.percentage}%` }} />
      </tr>
      {isOpen && <SkillBreakdown player={player} color={color} />}
    </Fragment>
  );
};
