import { useMeterSettingsStore } from "@/stores/useMeterSettingsStore";
import { EncounterState, EncounterUpdateEvent, PartyUpdateEvent, PlayerData, SortDirection, SortType } from "@/types";
import { usePrevious } from "@mantine/hooks";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

const DEFAULT_ENCOUNTER_STATE: EncounterState = {
  totalDamage: 0,
  dps: 0,
  startTime: 0,
  endTime: 1,
  party: {},
  targets: {},
  status: "Waiting",
};

export default function useMeter() {
  const { t } = useTranslation();
  const [currentTime, setCurrentTime] = useState(0);
  const [partyData, setPartyData] = useState<Array<PlayerData | null>>([null, null, null, null]);
  const [encounterState, setEncounterState] = useState<EncounterState>(DEFAULT_ENCOUNTER_STATE);
  const [lastPartyData, setLastPartyData] = useState<Array<PlayerData | null>>([null, null, null, null]);

  const previousStatus = usePrevious(encounterState.status);

  const [sortType, setSortType] = useState<SortType>("damage");
  const [sortDirection, setSortDirection] = useState<SortDirection>("desc");
  const { transparency } = useMeterSettingsStore(
    useShallow((state) => ({
      transparency: state.transparency,
    }))
  );

  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(Date.now());
    }, 500);

    return () => {
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    const encounterUpdateListener = listen("encounter-update", (event: EncounterUpdateEvent) => {
      setEncounterState(event.payload);

      if (event.payload.status === "InProgress" && encounterState.status === "Waiting") {
        encounterState.startTime == Date.now();
      }
    });

    const encounterSavedListener = listen("encounter-saved", () => {
      toast.success(t("ui.successful-save"));
    });

    const encounterSavedErrorListener = listen("encounter-saved-error", (evt) => {
      toast.error(t("ui.unsuccessful-save", { error: evt.payload }));
    });

    const onAreaEnterListener = listen("on-area-enter", (event: EncounterUpdateEvent) => {
      if (event.payload.status === "Waiting") {
        setEncounterState(DEFAULT_ENCOUNTER_STATE);
      } else {
        setEncounterState(event.payload);
      }

      toast.success(t("ui.on-area-enter"));
    });

    const onPartyUpdate = listen("encounter-party-update", (event: PartyUpdateEvent) => {
      setPartyData(event.payload);
    });

    const onSuccessAlert = listen("success-alert", (evt) => {
      toast.success(evt.payload as string);
    });

    const onErrorAlert = listen("error-alert", (evt) => {
      toast.error(evt.payload as string);
    });

    const onPinned = listen("on-pinned", (evt) => {
      evt.payload ? toast.success(t("ui.on-pin-enabled")) : toast.success(t("ui.on-pin-disabled"));
    });

    const onClickthrough = listen("on-clickthrough", (evt) => {
      evt.payload ? toast.success(t("ui.on-clickthrough-enabled")) : toast.success(t("ui.on-clickthrough-disabled"));
    });

    return () => {
      encounterUpdateListener.then((f) => f());
      encounterSavedListener.then((f) => f());
      encounterSavedErrorListener.then((f) => f());
      onAreaEnterListener.then((f) => f());
      onPartyUpdate.then((f) => f());
      onSuccessAlert.then((f) => f());
      onErrorAlert.then((f) => f());
      onPinned.then((f) => f());
      onClickthrough.then((f) => f());
    };
  }, [partyData]);

  useEffect(() => {
    if (previousStatus === "InProgress" && encounterState.status === "Stopped") {
      setLastPartyData(partyData);
    }
  }, [previousStatus, encounterState.status, partyData]);

  const elapsedTime = Math.max(currentTime - encounterState.startTime, 0);

  return {
    encounterState,
    partyData,
    lastPartyData,
    elapsedTime,
    sortType,
    setSortType,
    sortDirection,
    setSortDirection,
    transparency,
  };
}
