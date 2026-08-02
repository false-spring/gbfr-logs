// Hand-maintained display-name overrides, applied over the generated `enemies`
// name table as it loads. Keys are actor class names, lowercased.
import overridesFile from "@/assets/enemy-name-overrides.json";

const EnemyNameOverrides: Record<string, string> = Object.fromEntries(
  Object.entries(overridesFile.overrides).map(([key, name]) => [key.toLowerCase(), name])
);

type EnemyNameRow = { key?: string; text?: string };

export const applyEnemyNameOverrides = <T extends Record<string, EnemyNameRow>>(
  table: T
): { table: T; unmatched: string[] } => {
  const matched = new Set<string>();

  for (const row of Object.values(table)) {
    const key = row?.key?.toLowerCase();
    if (!key) continue;

    const override = EnemyNameOverrides[key];
    if (override === undefined) continue;

    row.text = override;
    matched.add(key);
  }

  return {
    table,
    unmatched: Object.keys(EnemyNameOverrides).filter((key) => !matched.has(key)),
  };
};

export default EnemyNameOverrides;
