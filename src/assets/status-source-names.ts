// What applied a status. Source ids are not unique on their own, so keys are
// "<sourceId>:<kindId>" pairs.
//
// Source id 0 must never appear here: it is the ABSENCE of an id, so a
// "0:<kind>" row names every character's unattributed status of that kind.
const StatusSourceNames: Record<string, string> = {
  "130:21": "Thunderwolf's Recharge",
  "10000:8": "Spirit Edge's Warpath",
  "10000:17": "Spirit Edge's Warpath",
  "10000:18": "Spirit Edge's Warpath",
  "10000:42": "Spirit Edge's Warpath",
  "10002:0": "In a Pinch",
  "10002:42": "In a Pinch",
  "10002:56": "In a Pinch",
  "11000:27": "Mage's Warpath",
  "11000:42": "Mage's Warpath",

  "99996:0": "Guard Payback",
  "10000:48": "Ultramarine's Warpath",
  "10000:49": "Ultramarine's Warpath",
  "10001:125": "Immortal Shell",
  "10000:4": "Crabvestment Returns",

  "10000:47": "Ultramarine's Warpath",
};

// Unconfirmed guesses, rendered with a trailing "?" in the UI.
export const StatusSourceGuesses: Record<string, string> = {
  // Demoted from the table above: the evidence shows this trait CAN produce a
  // source-less DMG↑, not that nothing else does.
  "0:42": "Ebony's Warpath",
  "0:16": "Potent Greens",
  "0:17": "Potent Greens",
  "0:26": "Mage's Warpath",
  "0:41": "Gamma",
  "105:42": "Helmsman's Warpath",
  "170:56": "Enchantress's Rhythm",
  "200:0": "Holy Knight's Grandeur",
  "200:3": "Founder's Strategy",
  "200:6": "Holy Knight's Grandeur",
  "200:7": "Dark Huntress's Volley",
  "200:41": "Bladequeen's Serenade",
  "210:56": "The Black's Impulse",
  "950:42": "The Black's Warpath",
  "2000:8": "Versalis Ignition",
  "2000:10": "Versalis Heart",
  "2000:18": "Versalis Ignition",
  "2000:42": "Versalis Heart",
  "2101:42": "Versalis Heart",
  "9995:7": "Butterfly's Warpath",
  "9995:42": "Butterfly's Warpath",
  "9996:0": "Lord's Ambition",
  "10000:24": "Hero's Creed",
  "99998:6": "Nimble Onslaught",
  "99999:51": "Flight over Fight",
};

// Wins over the per-character skill table, for skills that grant a status only
// because a trait is equipped. Keys are "<characterType>:<kindId>:<sourceId>",
// character uppercased.
export const StatusSourceOverrides: Record<string, string> = {
  "PL1100:1:170": "Dragonslayer's Dominance",
  "PL1000:7:9996": "Lord's Warpath",
  "PL1000:0:9996": "Lord's Ambition",

  // Cagliostro's DMG↑ arrives with no source id at all. Scoped to Pl1800 rather
  // than put back in the pair table, since the key has to carry the character.
  "PL1800:42:0": "Founder's Warpath",
};

export default StatusSourceNames;
