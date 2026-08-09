import { CharacterType, ComputedSbaSourceState, SkillMeterColumns } from "@/types";
import { isAttributedCause } from "@/utils/derive";
import { getSbaCauseName } from "@/utils/i18n";
import { getSbaColumnValue } from "./skillColumns";

export type SbaRowProps = {
  characterType: CharacterType;
  source: ComputedSbaSourceState;
  color: string;
  columns: SkillMeterColumns[];
  nested?: boolean;
};

export const SbaRow = ({ characterType, source, color, columns, nested }: SbaRowProps) => {
  const attributed = isAttributedCause(source.cause);

  return (
    <tr className={`skill-row ${nested ? "nested" : ""} ${attributed ? "" : "unattributed"}`}>
      <td className={`text-left row-data ${nested ? "nested" : ""}`}>
        {getSbaCauseName(characterType, source.cause, source.childCharacterType)}
      </td>
      {columns.map((column) => {
        const columnValue = getSbaColumnValue(column, source);

        return (
          <td key={column} className="text-center row-data">
            {columnValue.value}
            <span className="unit font-sm">{columnValue.unit}</span>
          </td>
        );
      })}
      <div
        className="damage-bar"
        style={{
          backgroundColor: color,
          width: `${source.percentage}%`,
          opacity: attributed ? undefined : 0.35,
        }}
      />
    </tr>
  );
};
