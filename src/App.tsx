import { appWindow } from "@tauri-apps/api/window";
import { Minus, Lightning } from "@phosphor-icons/react";

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

const Table = () => {
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
        <tr className="player-row">
          <td className="text-left">1 - Cagliostro</td>
          <td className="text-center">
            10<span className="unit">m</span>
          </td>
          <td className="text-center">
            327.7<span className="unit">k</span>
          </td>
          <td className="text-center">
            36.2<span className="unit">%</span>
          </td>
          <div
            className="damage-bar"
            style={{ backgroundColor: "#FF5630", width: "36.2%" }}
          />
        </tr>
        <tr className="player-row">
          <td className="text-left">2 - Siegfried</td>
          <td className="text-center">
            7.7<span className="unit">m</span>
          </td>
          <td className="text-center">
            270.2<span className="unit">k</span>
          </td>
          <td className="text-center">
            27.9<span className="unit">%</span>
          </td>
          <div
            className="damage-bar"
            style={{ backgroundColor: "#FFAB00", width: "27.9%" }}
          />
        </tr>
        <tr className="player-row">
          <td className="text-left">3 - Eugen</td>
          <td className="text-center">
            7.7<span className="unit">m</span>
          </td>
          <td className="text-center">
            270.2<span className="unit">k</span>
          </td>
          <td className="text-center">
            27.9<span className="unit">%</span>
          </td>
          <div
            className="damage-bar"
            style={{ backgroundColor: "#36B37E", width: "27.9%" }}
          />
        </tr>
        <tr className="player-row">
          <td className="text-left">4 - Io</td>
          <td className="text-center">
            2.2<span className="unit">m</span>
          </td>
          <td className="text-center">
            50.5<span className="unit">k</span>
          </td>
          <td className="text-center">
            8<span className="unit">%</span>
          </td>
          <div
            className="damage-bar"
            style={{ backgroundColor: "#00B8D9", width: "8%" }}
          />
        </tr>
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
