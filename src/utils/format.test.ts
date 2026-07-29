import { describe, expect, it } from "vitest";
import { millisecondsToElapsedPreciseFormat, toHash, toHashString } from "./format";

describe("format", () => {
  it("toHash", () => {
    expect(toHash(1)).toBe("1");
    expect(toHash(255)).toBe("ff");
  });

  it("toHashString", () => {
    expect(toHashString(1)).toBe("00000001");
    expect(toHashString(255)).toBe("000000ff");
  });
});

describe("millisecondsToElapsedPreciseFormat", () => {
  it("keeps the milliseconds, zero-padded", () => {
    expect(millisecondsToElapsedPreciseFormat(13_239)).toBe("00:13.239");
    expect(millisecondsToElapsedPreciseFormat(4_300)).toBe("00:04.300");
    expect(millisecondsToElapsedPreciseFormat(3_028)).toBe("00:03.028");
    expect(millisecondsToElapsedPreciseFormat(3_000)).toBe("00:03.000");
  });

  it("rolls over into minutes", () => {
    expect(millisecondsToElapsedPreciseFormat(83_456)).toBe("01:23.456");
  });
});
