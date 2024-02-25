import { appWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Minus, Lightning } from "@phosphor-icons/react";

import { useTranslation } from "react-i18next";

import "./i18n";
import "./App.css";
import { useEffect, useState } from "react";

const Titlebar = () => {
  const onMinimize = () => {
    appWindow.hide();
  };

  return (
    <div data-tauri-drag-region className="titlebar">
      <div
        className="titlebar-button"
        id="titlebar-minimize"
        onClick={onMinimize}
      >
        <Minus size={16} />
      </div>
    </div>
  );
};

type PlayerData = {
  index: number;
  // @TODO(false): Handle unknown CharacterTypes
  character_type: string;
  total_damage: number;
  dps: number;
  last_damage_time: number;

  // Calculated fields
  percentage: number;
};

type EncounterUpdateEvent = {
  event: string;
  payload: EncounterState;
};

type EncounterState = {
  total_damage: number;
  dps: number;
  start_time: number;
  end_time: number;
  party: Record<string, PlayerData>;
};

const PlayerRow = ({
  player,
  color,
}: {
  player: PlayerData;
  color: string;
}) => {
  const { t } = useTranslation();

  return (
    <tr className="player-row">
      <td className="text-left">
        {player.index} - {t(`characters.${player.character_type}`)}
      </td>
      <td className="text-center">{player.total_damage}</td>
      <td className="text-center">{Math.round(player.dps)}</td>
      <td className="text-center">
        {player.percentage.toFixed(2)}
        <span className="unit">%</span>
      </td>
      <div
        className="damage-bar"
        style={{ backgroundColor: color, width: `${player.percentage}%` }}
      />
    </tr>
  );
};

const Table = ({ encounterState }: { encounterState: EncounterState }) => {
  const colors = ["#FF5630", "#FFAB00", "#36B37E", "#00B8D9"];

  let players = Object.keys(encounterState.party).map((key) => {
    let playerData = encounterState.party[key];
    playerData.percentage =
      (playerData.total_damage / encounterState.total_damage) * 100;

    return playerData;
  });

  players.sort((a, b) => b.total_damage - a.total_damage);

  return (
    <table className="table w-full">
      <thead className="header">
        <tr>
          <th className="header-name">Name</th>
          <th className="header-column text-center">DMG</th>
          <th className="header-column text-center">DPS</th>
          <th className="header-column text-center">%</th>
        </tr>
      </thead>
      <tbody>
        {players.map((player, index) => (
          <PlayerRow key={index} player={player} color={colors[index]} />
        ))}
      </tbody>
    </table>
  );
};

const Footer = () => {
  return (
    <div className="footer">
      <div className="version">
        GBFR Logs <span className="version-number">v0.0.0</span>
      </div>
      <div className="status">
        <div className="status-text">Connected</div>
        <div className="status-icon">
          <Lightning size={16} fill={"#58e777"} />
        </div>
      </div>
    </div>
  );
};

function App() {
  const [encounterState, setEncounterState] = useState<EncounterState>({
    total_damage: 0,
    dps: 0,
    start_time: 0,
    end_time: 1,
    party: {},
  });

  useEffect(() => {
    listen("encounter-update", (event: EncounterUpdateEvent) => {
      setEncounterState(event.payload);
    });
  }, []);

  return (
    <div className="app">
      <Titlebar />
      <div className="app-content">
        <Table encounterState={encounterState} />
      </div>
      <Footer />
    </div>
  );
}

export default App;
