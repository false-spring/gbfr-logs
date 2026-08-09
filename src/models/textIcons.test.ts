import { describe, expect, it } from "vitest";

import { resolveTextIcons, textIconGlyph } from "@/models/textIcons";

describe("textIconGlyph", () => {
  it("renders the two attack buttons as their pad face buttons", () => {
    expect(textIconGlyph(4)).toBe("X");
    expect(textIconGlyph(3)).toBe("Y");
  });

  it("keeps the gem-rank pip as a diamond", () => {
    expect(textIconGlyph(1700)).toBe("◆");
  });

  it("drops status and sigil icons, whose name is already in the text", () => {
    expect(textIconGlyph(300)).toBe("");
    expect(textIconGlyph(1500)).toBe("");
  });
});

describe("resolveTextIcons", () => {
  it("leaves unmarked text untouched", () => {
    expect(resolveTextIcons("Critical Hit Rate +25%")).toBe("Critical Hit Rate +25%");
  });

  it("renders a button marker", () => {
    expect(resolveTextIcons("{icon:4} Attacks:\nDMG Cap +35%")).toBe("X Attacks:\nDMG Cap +35%");
  });

  it("renders both attack buttons in a pair", () => {
    expect(resolveTextIcons("{icon:4} / {icon:3} Attacks")).toBe("X / Y Attacks");
  });

  it("keeps repeated rank pips", () => {
    expect(resolveTextIcons("Crux Rank {icon:1700}{icon:1700}:")).toBe("Crux Rank ◆◆:");
  });

  it("drops a status marker without leaving a double space", () => {
    expect(resolveTextIcons("ATK↑ {icon:300} grants an additional ATK +10%")).toBe(
      "ATK↑ grants an additional ATK +10%"
    );
  });

  it("does not strand a space before punctuation", () => {
    expect(resolveTextIcons("Grants Supplementary DMG (5%) {icon:304}.")).toBe("Grants Supplementary DMG (5%).");
  });

  it("drops an unresolved non-icon substitution", () => {
    expect(resolveTextIcons("Attack +10{dyn:16}+20{dyn:17}")).toBe("Attack +10+20");
  });

  it("leaves the value placeholders the game uses alone", () => {
    expect(resolveTextIcons("DMG Cap +{0}% per {icon:1480} Basic Stats-type sigil")).toBe(
      "DMG Cap +{0}% per Basic Stats-type sigil"
    );
  });
});
