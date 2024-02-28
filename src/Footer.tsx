import { EncounterState } from "./types";
import { millisecondsToElapsedFormat } from "./utils";

const EncounterStatus = ({
  encounterState,
  elapsedTime,
}: {
  encounterState: EncounterState;
  elapsedTime: number;
}) => {
  if (encounterState.status === "Waiting") {
    return <div className="encounter-status">{encounterState.status}..</div>;
  } else if (encounterState.status === "InProgress") {
    return (
      <div className="encounter-elapsedTime">
        {millisecondsToElapsedFormat(elapsedTime)}
      </div>
    );
  } else if (encounterState.status === "Stopped") {
    return (
      <div className="encounter-elapsedTime">
        {millisecondsToElapsedFormat(
          encounterState.endTime - encounterState.startTime
        )}
      </div>
    );
  }
};

export const Footer = ({
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
      <EncounterStatus
        encounterState={encounterState}
        elapsedTime={elapsedTime}
      />
    </div>
  );
};
