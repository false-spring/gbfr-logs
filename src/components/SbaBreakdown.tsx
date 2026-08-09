import { useTranslation } from "react-i18next";

import { ComputedPlayerState, ComputedSbaGroup } from "@/types";
import { SbaGroupRow } from "./SbaGroupRow";
import { SbaRow } from "./SbaRow";
import { SBA_SKILL_COLUMNS, SKILL_COLUMN_LABEL } from "./skillColumns";
import { useSbaBreakdown } from "./useSbaBreakdown";

export type SbaBreakdownProps = {
  player: ComputedPlayerState;
  color: string;
};

export const SbaBreakdown = ({ player, color }: SbaBreakdownProps) => {
  const { t } = useTranslation();
  const { sources } = useSbaBreakdown(player);

  return (
    <tr className="skill-table">
      <td colSpan={100}>
        <table className="table w-full">
          <thead className="header transparent-bg">
            <tr>
              <th className="header-name">{t("ui.logs.sba-source")}</th>
              {SBA_SKILL_COLUMNS.map((column) => (
                <th key={column} className="header-column text-center">
                  {SKILL_COLUMN_LABEL[column]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="transparent-bg">
            {sources.map((source, index) => {
              const key = `${JSON.stringify(source.cause)}-${index}`;

              return "sources" in source ? (
                <SbaGroupRow
                  key={key}
                  characterType={player.characterType}
                  group={source as ComputedSbaGroup}
                  color={color}
                  columns={SBA_SKILL_COLUMNS}
                />
              ) : (
                <SbaRow
                  key={key}
                  characterType={player.characterType}
                  source={source}
                  color={color}
                  columns={SBA_SKILL_COLUMNS}
                />
              );
            })}
          </tbody>
        </table>
      </td>
    </tr>
  );
};
