import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(cleanup);

vi.mock("@/assets/skill-groups", () => ({ default: {} }));

const EN_METER_COLUMN_LABELS: Record<string, string> = {
  name: "Name",
  dps: "DPS",
  damage: "DMG",
  "damage-percentage": "%",
  sba: "SBA",
  "total-stun-value": "Stun",
  "stun-per-second": "SPS",
  "stun-per-hit": "Per Hit",
  "stun-percentage": "%",
  "heal-done": "Heal",
};
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => EN_METER_COLUMN_LABELS[key.replace("ui.meter-columns.", "")] ?? key,
  }),
}));

import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { EncounterState, MeterColumns } from "@/types";
import { Table } from "./Table";

useMeterSettingsStore.setState({ show_full_values: true });

const encounterState: EncounterState = {
  totalDamage: 1000,
  dps: 10,
  startTime: 0,
  endTime: 1000,
  status: "Stopped",
  targets: {},
  party: {
    "0": {
      index: 0,
      characterType: "Pl1000",
      totalDamage: 1000,
      dps: 10,
      sba: 0,
      totalStunValue: 50,
      stunPerSecond: 2,
      lastDamageTime: 0,
      skillBreakdown: [],
      totalDamageTaken: 0,
      healDone: 0,
      healReceived: 0,
      healProvidedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
      healReceivedByType: { skill: 0, selfRecovery: 0, regen: 0, revive: 0, other: 0 },
      sbaBreakdown: [],
      totalSbaAdded: 0,
    },
  },
};

const lopsidedEncounterState: EncounterState = {
  ...encounterState,
  party: {
    "0": { ...encounterState.party["0"], index: 0, totalDamage: 900, totalStunValue: 25 },
    "1": { ...encounterState.party["0"], index: 1, characterType: "Pl1100", totalDamage: 100, totalStunValue: 75 },
  },
};

const renderTable = (metric?: "damage" | "stun", live = false, state: EncounterState = encounterState) =>
  render(
    <Table
      live={live}
      encounterState={state}
      partyData={[]}
      sortType={MeterColumns.TotalDamage}
      sortDirection="desc"
      setSortType={() => {}}
      setSortDirection={() => {}}
      metric={metric}
    />
  );

describe("Table", () => {
  it("shows the full damage/stun column set by default", () => {
    renderTable();

    expect(screen.queryByText("DMG")).toBeTruthy();
    expect(screen.queryByText("DPS")).toBeTruthy();
    expect(screen.queryByText("Stun")).toBeTruthy();
    expect(screen.queryByText("SPS")).toBeTruthy();
    expect(screen.queryByText("Per Hit")).toBeFalsy();
  });

  it("shows only Stun / Per Hit / SPS / % for the stun metric — no DMG or DPS", () => {
    renderTable("stun");
    expect(screen.queryByText("Stun")).toBeTruthy();
    expect(screen.queryByText("Per Hit")).toBeTruthy();
    expect(screen.queryByText("SPS")).toBeTruthy();
    expect(screen.getAllByText("%").length).toBeGreaterThan(0);

    expect(screen.queryByText("DMG")).toBeFalsy();
    expect(screen.queryByText("DPS")).toBeFalsy();
  });

  it("renders the overlay's Stun % column as the party stun share, not the damage share", () => {
    useMeterSettingsStore.setState({ overlay_columns: [MeterColumns.StunPercentage] });
    renderTable(undefined, true, lopsidedEncounterState);

    expect(screen.queryByText("25")).toBeTruthy();
    expect(screen.queryByText("75")).toBeTruthy();
    expect(screen.queryByText("90")).toBeFalsy();
    expect(screen.queryByText("10")).toBeFalsy();
  });
});
