export const resolveEffectiveEnemy = (
  selectedEnemy: number | "all" | null,
  selectableIndices: number[]
): number | "all" | null => {
  if (selectedEnemy === "all") return "all";
  if (selectedEnemy !== null && selectableIndices.includes(selectedEnemy)) return selectedEnemy;
  return selectableIndices.length > 1 ? "all" : selectableIndices[0] ?? null;
};

export const sumChartsAcrossEnemies = (
  byEnemy: Record<number, Record<number, number[]>>,
  indices?: number[]
): Record<number, number[]> => {
  const perEnemy =
    indices === undefined
      ? Object.values(byEnemy)
      : indices.map((index) => byEnemy[index]).filter((buckets): buckets is Record<number, number[]> => !!buckets);
  const merged: Record<number, number[]> = {};
  for (const perPlayer of perEnemy) {
    for (const playerIndex in perPlayer) {
      const src = perPlayer[playerIndex];
      const dst = (merged[playerIndex] ??= []);
      for (let i = 0; i < src.length; i++) dst[i] = (dst[i] ?? 0) + src[i];
    }
  }
  return merged;
};
