import { describe, expect, it } from "vitest";

import StatusSourceNames, { StatusSourceGuesses, StatusSourceOverrides } from "./status-source-names";

describe("status source names", () => {
  /** Source id 0 is the ABSENCE of a source id, not a shared one, so a
   * `0:<kind>` row names every character's unattributed status of that kind
   * after whoever was captured first. */
  it("never names anything in the confirmed table from source id 0", () => {
    const offenders = Object.keys(StatusSourceNames).filter((key) => key.startsWith("0:"));
    expect(offenders).toEqual([]);
  });

  it("allows them in the guesses table, which marks its entries unconfirmed", () => {
    const sourceless = Object.keys(StatusSourceGuesses).filter((key) => key.startsWith("0:"));
    expect(sourceless.length).toBeGreaterThan(0);
  });

  it("keeps the demoted row as a lead rather than dropping its evidence", () => {
    expect(StatusSourceNames["0:42"]).toBeUndefined();
    expect(StatusSourceGuesses["0:42"]).toBe("Ebony's Warpath");
  });

  it("names a source-less status through the character-scoped table instead", () => {
    expect(StatusSourceOverrides["PL1800:42:0"]).toBe("Founder's Warpath");
  });

  /** `View.tsx` builds this key uppercased; a row spelled any other way is
   * silently dead. */
  it("keys every override on an uppercased character, a kind and a source id", () => {
    for (const key of Object.keys(StatusSourceOverrides)) {
      expect(key).toMatch(/^PL\d{4}:\d+:\d+$/);
    }
  });
});
