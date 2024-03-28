import { Toaster } from "react-hot-toast";

import { Table } from "@/components/Table";
import { Titlebar } from "@/components/Titlebar";
import "@/i18n";

import useMeter from "./useMeter";

export const Meter = () => {
  const {
    encounterState,
    partyData,
    lastPartyData,
    elapsedTime,
    sortType,
    setSortType,
    sortDirection,
    setSortDirection,
    transparency,
  } = useMeter();

  return (
    <div className="app">
      <Titlebar
        encounterState={encounterState}
        partyData={encounterState.status === "Stopped" ? lastPartyData : partyData}
        elapsedTime={elapsedTime}
        sortType={sortType}
        sortDirection={sortDirection}
      />
      <div className="app-content" style={{ background: `rgba(22, 22, 22, ${transparency})` }}>
        <Table
          live
          encounterState={encounterState}
          partyData={encounterState.status === "Stopped" ? lastPartyData : partyData}
          sortType={sortType}
          setSortType={setSortType}
          sortDirection={sortDirection}
          setSortDirection={setSortDirection}
        />
      </div>
      <Toaster
        position="bottom-center"
        toastOptions={{
          style: {
            borderRadius: "10px",
            backgroundColor: "#252525",
            color: "#fff",
            fontSize: "14px",
          },
        }}
      />
    </div>
  );
};
