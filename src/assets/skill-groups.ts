type SkillGroupMapping = {
  [key: string]: {
    [key: string]: {
      skills: number[];
    };
  };
};

const SkillGroups: SkillGroupMapping = {
  Pl0000: {
    "normal-attack": {
      skills: [100, 110, 120, 121, 130, 131],
    },
    "power-raise": {
      skills: [200, 201, 202, 203, 204, 205],
    },
    "power-strike": {
      skills: [210, 220, 230, 240],
    },
    "aerial-attack": {
      skills: [300, 310, 320],
    },
    "overdrive-surge": {
      skills: [1001, 1002, 1003, 1004],
    },
    decimate: {
      skills: [1201, 1202, 1203, 1204],
    },
    "rain-of-arrows": {
      skills: [1301, 1302, 1303, 1304],
    },
    "armor-break": {
      skills: [1501, 1502, 1503, 1504],
    },
    reginleiv: {
      skills: [2001, 2002, 2003, 2004],
    },
    dispel: {
      skills: [2410, 2411, 2412, 2413],
    },
  },
  Pl0100: {
    "normal-attack": {
      skills: [100, 110, 120, 121, 130, 131],
    },
    "power-raise": {
      skills: [200, 201, 202, 203, 204, 205],
    },
    "power-strike": {
      skills: [210, 220, 230, 240],
    },
    "aerial-attack": {
      skills: [300, 310, 320],
    },
    "overdrive-surge": {
      skills: [1001, 1002, 1003, 1004],
    },
    decimate: {
      skills: [1201, 1202, 1203, 1204],
    },
    "rain-of-arrows": {
      skills: [1301, 1302, 1303, 1304],
    },
    "armor-break": {
      skills: [1501, 1502, 1503, 1504],
    },
    reginleiv: {
      skills: [2001, 2002, 2003, 2004],
    },
    dispel: {
      skills: [2410, 2411, 2412, 2413],
    },
  },
  Pl0200: {
    "normal-attack": {
      skills: [100, 101, 102, 103, 104],
    },
    finisher: {
      skills: [110, 120, 130, 140],
    },
    "oathsworn-blade": {
      skills: [200, 201, 202],
    },
    pactstrike: {
      skills: [240, 241, 242, 243, 244, 245, 250, 251, 252, 253, 254, 255, 259],
    },
    "aerial-attack": {
      skills: [300, 301, 302, 303],
    },
  },
  Pl0300: {
    "aerial-attack": {
      skills: [300, 301, 302],
    },
  },
  Pl0400: {
    "normal-attack": {
      skills: [100, 110, 120],
    },
    stargaze: {
      skills: [210, 211, 212, 213, 214],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
    "flowery-seven": {
      skills: [4000, 4001],
    },
    "gravity-well": {
      skills: [7200, 7201, 7202, 7203],
    },
    lightning: {
      skills: [8200, 8201, 8202, 8203],
    },
  },
  Pl0500: {
    "normal-attack": {
      skills: [100, 105, 110, 111, 112],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
    detonator: {
      skills: [2, 22, 32],
    },
    grenade: {
      skills: [210, 215],
    },
  },
  Pl0600: {
    "normal-attack": {
      skills: [100, 101, 102, 103],
    },
    "aerial-attack": {
      skills: [300, 301],
    },
    "rose-attack": {
      skills: [200, 201, 202, 203, 204, 1],
    },
    "spiral-rose": {
      skills: [4, 14, 24, 34],
    },
    "lost-love": {
      skills: [6, 16, 26, 36],
    },
  },
  Pl0700: {
    "normal-attack": {
      skills: [100, 102, 111, 110, 112, 101, 120, 122, 121],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
  },
  Pl0800: {
    "normal-attack": {
      skills: [100, 101, 102],
    },
    "stiller-glantz": {
      skills: [103, 104, 105, 106, 107, 108, 109],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
  },
  Pl0900: {
    "normal-attack": {
      skills: [100, 101, 102],
    },
    "power-strike": {
      skills: [110, 120, 103],
    },
    finisher: {
      skills: [111, 121, 104],
    },
    "heavy-swing": {
      skills: [200, 201, 202, 203, 204],
    },
    "aerial-attack": {
      skills: [300, 310, 320],
    },
  },
  Pl1000: {
    "normal-attack": {
      skills: [100, 110, 120],
    },
    "aerial-attack": {
      skills: [300, 310, 320],
    },
    schlaht: {
      skills: [200, 201],
    },
  },
  Pl1100: {
    "normal-attack": {
      skills: [100, 101, 102, 103, 111, 112, 122, 162, 113, 123, 163],
    },
    "combo-finisher": {
      skills: [104, 124, 150, 164, 114],
    },
    lunge: {
      skills: [200, 201, 202, 203],
    },
    "aerial-attack": {
      skills: [300, 301, 311, 312, 313],
    },
  },
  Pl1200: {
    "normal-attack": {
      skills: [100, 101, 102],
    },
    "noble-stance": {
      skills: [103, 104, 105, 106, 107],
    },
    "yellow-power-strike": {
      skills: [120, 130],
    },
    "power-strike": {
      skills: [201, 202],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
    "sword-of-lumiel": {
      skills: [1600, 1610],
    },
  },
  Pl1300: {
    "normal-attack": {
      skills: [100, 110, 120, 130, 140],
    },
    "flashing-void": {
      skills: [17, 18],
    },
    "sharpened-focus": {
      skills: [111, 121, 131, 141],
    },
    finisher: {
      skills: [150, 151, 152, 153, 154],
    },
    "aerial-attack": {
      skills: [300, 301, 302, 303],
    },
  },
  Pl1400: {
    "freeflutter-attack": {
      skills: [100, 101, 102, 110, 111, 112, 113, 114],
    },
    "freeflutter-aerial-attack": {
      skills: [300, 310, 320],
    },
    "dawnfly-attack": {
      skills: [1100, 1103, 1106, 1108],
    },
    "freeflutter-finisher": {
      skills: [2000, 2001],
    },
    "dawnfly-charged-attack": {
      skills: [1109, 1150, 1151],
    },
    apex: {
      skills: [3, 10, 11],
    },
    transient: {
      skills: [5, 1005],
    },
  },
  Pl1500: {
    "normal-attack": {
      skills: [100, 101, 102, 103, 111, 112, 113, 122, 123],
    },
    "combo-finisher": {
      skills: [104, 114, 124],
    },
    "aerial-attack": {
      skills: [300, 301, 310],
    },
    "aerial-finisher": {
      skills: [302, 303, 311],
    },
  },
  Pl1600: {
    "normal-attack": {
      skills: [100, 101, 102],
    },
    "loop-combo": {
      skills: [110, 111, 210],
    },
    "aerial-attack": {
      skills: [300, 301, 302, 303],
    },
    "realms-majesty": {
      skills: [1700, 1702, 1703],
    },
  },
  Pl1700: {
    "normal-attack": {
      skills: [100, 110, 120, 114, 122],
    },
    "power-strike": {
      skills: [112, 113, 121, 130],
    },
    finisher: {
      skills: [280, 281, 251, 252],
    },
    "aerial-attack": {
      skills: [300, 301],
    },
  },
  Pl1800: {
    "normal-attack": {
      skills: [100, 101, 102, 103],
    },
    "power-strike": {
      skills: [110, 120, 130, 104],
    },
    finisher: {
      skills: [111, 121, 131, 105],
    },
    "enhanced-collapse": {
      skills: [210, 220, 230],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
  },
  Pl1900: {
    "normal-attack": {
      skills: [100, 101, 102],
    },
    lunge: {
      skills: [200, 201, 202, 203],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
    "power-strike": {
      skills: [503, 505, 506, 507, 508, 509, 510],
    },
  },
  Pl2000: {
    "normal-attack": {
      skills: [100, 101, 102, 103, 104, 105, 106, 107, 108, 109],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
  },
  Pl2200: {
    "normal-attack": {
      skills: [100, 101, 102, 103, 140, 141],
    },
    finisher: {
      skills: [104, 111, 121, 131],
    },
    "power-strike": {
      skills: [110, 120, 130],
    },
    "aerial-attack": {
      skills: [300, 301, 302],
    },
    "avatar-attack": {
      skills: [900, 901, 902, 903, 940, 941],
    },
    "avatar-finisher": {
      skills: [904, 911, 921, 931],
    },
    "avatar-power-strike": {
      skills: [910, 920, 930],
    },
  },
  Pl2300: {
    "normal-attack": {
      skills: [100, 110, 120],
    },
    "charged-normal-attack": {
      skills: [101, 111, 121],
    },
    "combo-finisher": {
      skills: [151, 160, 170],
    },
    "charged-combo-finisher": {
      skills: [152, 171],
    },
    "power-finisher": {
      skills: [155, 156, 165, 175, 176],
    },
    "aerial-attack": {
      skills: [300, 310, 320],
    },
    "two-crown-strife": {
      skills: [1700, 1701],
    },
  },
};

export default SkillGroups;
