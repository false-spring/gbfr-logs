import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";

import "./i18n";
import "./App.css";

import { EncounterState, EncounterUpdateEvent } from "./types";
import toast, { Toaster } from "react-hot-toast";
import { Footer } from "./Footer";
import { Table } from "./Table";
import { Titlebar } from "./Titlebar";

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

    const onAreaEnterListener = listen(
      "on-area-enter",
      (event: EncounterUpdateEvent) => {
        setEncounterState(event.payload);
        toast.success(t("ui.on-area-enter"));
      }
    );

    return () => {
      encounterUpdateListener.then((f) => f());
      encounterSavedListener.then((f) => f());
      encounterSavedErrorListener.then((f) => f());
      onAreaEnterListener.then((f) => f());
    };
  }, []);

  const elapsedTime = Math.max(currentTime - encounterState.startTime, 0);

  console.log(encounterState.status);

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
