import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { appWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Minus, Camera } from "@phosphor-icons/react";

import "./i18n";
import "./App.css";

import {
  ComputedPlayerData,
  EncounterState,
  EncounterUpdateEvent,
} from "./types";
import {
  humanizeNumbers,
  millisecondsToElapsedFormat,
  exportScreenshotToClipboard,
} from "./utils";
import toast, { Toaster } from "react-hot-toast";

const Titlebar = () => {
  const onMinimize = () => {
    appWindow.minimize();
  };

  return (
    <div data-tauri-drag-region className="titlebar transparent-bg font-sm">
      <div data-tauri-drag-region className="titlebar-left"></div>
      <div data-tauri-drag-region className="titlebar-right">
        <div
          className="titlebar-button"
          id="titlebar-snapshot"
          onClick={exportScreenshotToClipboard}
        >
          <Camera size={16} />
        </div>
        <div
          className="titlebar-button"
          id="titlebar-minimize"
          onClick={onMinimize}
        >
          <Minus size={16} />
        </div>
      </div>
    </div>
  );
};

const PlayerRow = ({
  player,
  color,
}: {
  player: ComputedPlayerData;
  color: string;
}) => {
  const { t } = useTranslation();

  const [totalDamage, totalDamageUnit] = humanizeNumbers(player.totalDamage);
  const [dps, dpsUnit] = humanizeNumbers(player.dps);

  return (
    <tr className="player-row">
      <td className="text-left row-data">
        {player.index} - {t(`characters.${player.characterType}`)}
      </td>
      <td className="text-center row-data">
        {totalDamage}
        <span className="unit font-sm">{totalDamageUnit}</span>
      </td>
      <td className="text-center row-data">
        {dps}
        <span className="unit font-sm">{dpsUnit}</span>
      </td>
      <td className="text-center row-data">
        {player.percentage.toFixed(2)}
        <span className="unit font-sm">%</span>
      </td>
      <div
        className="damage-bar"
        style={{ backgroundColor: color, width: `${player.percentage}%` }}
      />
    </tr>
  );
};

const Table = ({ encounterState }: { encounterState: EncounterState }) => {
  const colors = [
    "#FF5630",
    "#FFAB00",
    "#36B37E",
    "#00B8D9",
    "#9BCF53",
    "#380E7F",
    "#416D19",
    "#2C568D",
  ];

  let players: Array<ComputedPlayerData> = Object.keys(
    encounterState.party
  ).map((key) => {
    let playerData = encounterState.party[key];

    return {
      percentage: (playerData.totalDamage / encounterState.totalDamage) * 100,
      ...playerData,
    };
  });

  players.sort((a, b) => b.totalDamage - a.totalDamage);

  return (
    <table className="table w-full">
      <thead className="header transparent-bg">
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

const Footer = ({
  encounterState,
  elapsedTime,
}: {
  encounterState: EncounterState;
  elapsedTime: number;
}) => {
  return (
    <div className="footer transparent-bg font-sm">
      <div className="version">
        GBFR Logs <span className="version-number">0.0.2</span>
      </div>

      {encounterState.status === "Waiting" ? (
        <div className="encounter-status">{encounterState.status}..</div>
      ) : (
        <div className="encounter-elapsedTime">
          {millisecondsToElapsedFormat(elapsedTime)}
        </div>
      )}
    </div>
  );
};

const App = () => {
  const { t } = useTranslation();
  const [currentTime, setCurrentTime] = useState(0);
  const [encounterState, setEncounterState] = useState<EncounterState>({
    totalDamage: 0,
    dps: 0,
    startTime: 0,
    endTime: 1,
    party: {},
    status: "Waiting",
  });

  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(Date.now());
    }, 500);

    return () => {
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    const encounterUpdateListener = listen(
      "encounter-update",
      (event: EncounterUpdateEvent) => {
        setEncounterState(event.payload);

        if (
          event.payload.status === "InProgress" &&
          encounterState.status === "Waiting"
        ) {
          encounterState.startTime == Date.now();
        }
      }
    );

    const encounterSavedListener = listen("encounter-saved", () => {
      toast.success(t("ui.successful-save"));
    });

    const encounterSavedErrorListener = listen(
      "encounter-saved-error",
      (evt) => {
        toast.error(t("ui.unsuccessful-save", { error: evt.payload }));
      }
    );

    const encounterResetListener = listen(
      "encounter-reset",
      (event: EncounterUpdateEvent) => {
        setEncounterState(event.payload);
      }
    );

    const onAreaEnterListener = listen("on-area-enter", () => {
      toast.success(t("ui.on-area-enter"));
    });

    return () => {
      encounterUpdateListener.then((f) => f());
      encounterResetListener.then((f) => f());
      encounterSavedListener.then((f) => f());
      encounterSavedErrorListener.then((f) => f());
      onAreaEnterListener.then((f) => f());
    };
  }, []);

  const elapsedTime = Math.max(currentTime - encounterState.startTime, 0);

  return (
    <div className="app">
      <Titlebar />
      <div className="app-content">
        <Table encounterState={encounterState} />
      </div>
      <Toaster
        position="bottom-center"
        toastOptions={{
          style: {
            borderRadius: "10px",
            backgroundColor: "#252525",
            color: "#fff",
          },
        }}
      />
      <Footer encounterState={encounterState} elapsedTime={elapsedTime} />
    </div>
  );
};

export default App;
