/**
 * Statuses granted by MASTER TRAITS, keyed by character and status kind, and
 * discriminated by the MAGNITUDE the node declares.
 *
 * Master traits are not identified the way sigils and skills are. They are not
 * hash-probed and carry no resolvable source id, so a status they grant reaches
 * the meter with nothing naming it.
 *
 * The tables do declare which node grants which status, and attributing on that
 * alone -- "this player holds the only node of theirs granting Shield, so their
 * Shield came from it" -- was tried and FAILS. Every Shield across three
 * captures came from elsewhere: three from Pl1800's Reinforce (source id 1300,
 * which resolves normally) at 79135 / 33331 / 87235 points, one at 99999. The
 * perfect-dodge node exists for every character and grants Shield (1000), so
 * presence alone would have claimed all four.
 *
 * The declared VALUE separates them, which is why entries carry one. A
 * 1000-point shield on a character holding that node is the node's; a
 * 79135-point one is not.
 *
 * `value` is in the TABLE's units -- percent for a kind whose magnitude is a
 * fraction (10 means 10%, which the meter stores as 0.10), absolute for a count.
 * The consumer normalises before comparing.
 *
 * `label` is the node's trigger clause, taken from the game's own description:
 * these read "<when>: <what it grants>", and the "when" is what identifies the
 * perk to a player. A node hash or a board name would not. Where a node has no
 * usable trigger clause, the label is its board CARD's name instead.
 *
 * SEVERAL CANDIDATES MAY SHARE A VALUE, and that is not a defect to be cleaned
 * up. Cagliostro's Disruption and her Crippling Combos both inflict ATK DOWN at
 * 10%, so an observed 10% is genuinely either one; the consumer joins the labels
 * rather than picking. Keeping only the first is what made a Crippling Combos
 * debuff read as "Disruption".
 *
 * NOT AN EXHAUSTIVE INDEX of the nodes that grant a status. Some declare no
 * StatusId and are recovered from their description text, which reaches the ones
 * that name a magnitude and misses any that do not.
 *
 * Generated from the game's skillboard tables. Prerequisite rows ("While
 * inflicted with X: ...") are excluded -- they gate an effect rather than
 * inflicted with X: ..." form and any trigger clause naming the row's own status
 * ("While Shield is active" on a Shield row), which gate an effect rather than
 * granting one.
 */
export type MasterTraitSource = { value: number; label: string };

const StatusMasterTraitSources: Record<string, MasterTraitSource[]> = {
  "PL0000:3": [{ value: 10, label: "Dispel" }],
  "PL0000:7": [
    { value: 10, label: "Veil" },
    { value: 20, label: "Crux: The Substitute" },
  ],
  "PL0000:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL0100:3": [{ value: 10, label: "Dispel" }],
  "PL0100:7": [
    { value: 10, label: "Veil" },
    { value: 20, label: "Crux: The Substitute" },
  ],
  "PL0100:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL0200:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 1000, label: "Upon summoning Ares" },
  ],
  "PL0300:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 50000, label: "Crux: Collateral Risk" },
  ],
  "PL0400:1": [{ value: 10, label: "Mystic Vortex" }],
  "PL0400:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 3000, label: "Crux: Flowery Seven Overload" },
  ],
  "PL0500:2": [{ value: 10, label: "Disruptor" }],
  "PL0500:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL0600:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 3000, label: "Insight: Rose Requiem" },
  ],
  "PL0700:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 1000, label: "Whenever a pet is summoned" },
    { value: 2000, label: "Crux: Spirited Support" },
  ],
  "PL0800:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 10000, label: "Crux: Skill Acceleration" },
  ],
  "PL0900:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 10000, label: "Insight: Growing Hostility" },
    { value: 30000, label: "Insight: Growing Hostility" },
  ],
  "PL1000:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1100:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1200:41": [
    { value: 1000, label: "Upon blocking with a attack" },
    { value: 1000, label: "Upon performing a perfect dodge" },
  ],
  "PL1300:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1400:2": [{ value: 10, label: "Upon countering with Apex of Nothingness" }],
  "PL1400:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1500:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1600:41": [
    { value: 200, label: "Upon landing Arvess Hammer" },
    { value: 1000, label: "Upon performing a perfect dodge" },
  ],
  "PL1700:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1800:1": [{ value: 5, label: "Rhizomata" }],
  "PL1800:2": [
    { value: 10, label: "Disruption" },
    { value: 10, label: "Insight: Crippling Combos" },
  ],
  "PL1800:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL1900:0": [
    { value: 5, label: "Ragnarok Form" },
    { value: 10, label: "Never Enough" },
    { value: 15, label: "Ragnarok Form" },
    { value: 20, label: "Crux: Path of Vengeance" },
  ],
  "PL1900:3": [{ value: 10, label: "Arcadia" }],
  "PL1900:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL2100:7": [
    { value: 2, label: "Essence: Finding Refuge" },
    { value: 10, label: "Crux: Bell Tolls for Thee" },
  ],
  "PL2100:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL2100:45": [{ value: 10, label: "Crux: Bell Tolls for Thee" }],
  "PL2200:7": [{ value: 10, label: "Ispirazione" }],
  "PL2200:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL2300:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL2300:43": [{ value: 5, label: "Insight: Hail to Multilock" }],
  "PL2400:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
  "PL2500:41": [
    { value: 1000, label: "Essence: Superlative Sorcery" },
    { value: 1000, label: "Upon performing a perfect dodge" },
  ],
  "PL2600:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 3000, label: "While Delta Clock is in during Devour Causality" },
  ],
  "PL2700:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 5000, label: "Insight: Keep Your Foes Closer" },
  ],
  "PL2800:41": [
    { value: 1000, label: "Upon performing a perfect dodge" },
    { value: 3000, label: "Essence: Allure of Strength" },
  ],
  "PL2900:41": [{ value: 1000, label: "Upon performing a perfect dodge" }],
};

export default StatusMasterTraitSources;
