import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import EnemyNameOverrides, { applyEnemyNameOverrides } from "@/assets/enemy-name-overrides";
import overridesFile from "@/assets/enemy-name-overrides.json";

const shippedEnemies = (): Record<string, { key: string; text: string }> =>
  JSON.parse(readFileSync(resolve(process.cwd(), "src-tauri/lang/en/enemies.json"), "utf-8"));

describe("applyEnemyNameOverrides", () => {
  it("rewrites a matching row's text and leaves everything else alone", () => {
    const { table } = applyEnemyNameOverrides({
      "37a72b81": { key: "Em7700Trial8_11Crystal", text: "Em7700Trial8_11Crystal" },
      "2b31654b": { key: "Em7700", text: "Lucilius" },
    });

    expect(table["37a72b81"].text).toBe("Wings of Sin (Orbs)");
    expect(table["2b31654b"].text).toBe("Lucilius");
  });

  it("matches the class name case-insensitively", () => {
    const { table } = applyEnemyNameOverrides({
      "10fd87b2": { key: "we8470", text: "We8470" },
    });

    expect(table["10fd87b2"].text).toBe("Sky Dragon's Trial");
  });

  it("reports overrides that matched no row", () => {
    const { unmatched } = applyEnemyNameOverrides({
      "10fd87b2": { key: "WE8470", text: "We8470" },
    });

    expect(unmatched).toContain("em7700trial8_11crystal");
    expect(unmatched).not.toContain("we8470");
  });
});

describe("the override table itself", () => {
  it("names only classes the shipped en table actually carries", () => {
    const { unmatched } = applyEnemyNameOverrides(shippedEnemies());

    expect(unmatched).toEqual([]);
  });

  it("does not collide two spellings of one class onto different names", () => {
    const written = Object.keys(overridesFile.overrides);

    expect(Object.keys(EnemyNameOverrides).length).toBe(written.length);
  });
});
