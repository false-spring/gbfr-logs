import { appWindow } from "@tauri-apps/api/window";
import { Minus, Lightning } from "@phosphor-icons/react";

import { useTranslation } from "react-i18next";

import "./i18n";
import "./App.css";

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
  type: string;
  damage: number;
  dps: number;
  percentage: number;
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
        {player.index} - {t(`characters.${player.type}`)}
      </td>
      <td className="text-center">
        {player.damage}
        <span className="unit">m</span>
      </td>
      <td className="text-center">
        {player.dps}
        <span className="unit">k</span>
      </td>
      <td className="text-center">
        {player.percentage}
        <span className="unit">%</span>
      </td>
      <div
        className="damage-bar"
        style={{ backgroundColor: color, width: `${player.percentage}%` }}
      />
    </tr>
  );
};

const Table = () => {
  const colors = ["#FF5630", "#FFAB00", "#36B37E", "#00B8D9"];
  const playerData: PlayerData[] = [
    {
      index: 1,
      type: "PL1800",
      damage: 6.7,
      dps: 300,
      percentage: 36.2,
    },
    {
      index: 2,
      type: "PL1100",
      damage: 7.7,
      dps: 270.2,
      percentage: 27.9,
    },
    {
      index: 3,
      type: "PL0500",
      damage: 7.7,
      dps: 270.2,
      percentage: 27.9,
    },
    {
      index: 4,
      type: "PL0400",
      damage: 2.2,
      dps: 50.5,
      percentage: 8,
    },
  ];

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
        {playerData.map((player, index) => (
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
  return (
    <div className="app">
      <Titlebar />
      <div className="app-content">
        <Table />
      </div>
      <Footer />
    </div>
  );
}

export default App;
