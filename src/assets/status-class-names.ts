/**
 * Status kind id -> the game's own C++ class name, for the eleven kinds the
 * status table gives no display name to. These ids are constructible by the
 * status factory but have no `status.tbl` row, so there is no key, icon or
 * localized string anywhere in the data; without this they render as bare hex.
 *
 * Names are shown verbatim: a prettified label would be our invention with no
 * source to check it against. Read out of RTTI; re-derive with
 * `S:\gbfr-mining\tools\np_26_unnamed_status.py` on a game build change.
 *
 * Keys are kind ids, not the factory's row index, and ids are insertion-stable.
 */
export type StatusFallbackEntry = {
  className: string;
  name?: string;
  beneficial?: boolean;
};

export const StatusClassNames: Record<number, StatusFallbackEntry> = {
  0x0c: { className: "StatusManigansBuff", name: "Manigance", beneficial: true },
  0x0d: { className: "StatusPl1200AttackBuff", name: "Valiant Stance", beneficial: true },
  0x0e: { className: "StatusPl1300AttackBuff", name: "Trice Blade", beneficial: true },
  0x0f: { className: "StatusEm0001Buff" },
  0x10: { className: "StatusClearAilment", name: "Cleanse", beneficial: true },
  0x22: { className: "StatusEm1700AttackBuff" },
  0x23: { className: "StatusEm1700DefenceBuff" },
  0x27: { className: "StatusUniqueGutsBuff" },
  0x8f: { className: "StatusDimensionDamageInvalidBuff" },
  0x3f6: { className: "StatusAilmentProvocation" },
  // Goes on the ENEMY, so it groups with the debuffs.
  0x3fe: { className: "StatusAilmentDebuffTimeExtend", name: "Debuff Extension", beneficial: false },
};

export default StatusClassNames;
