import { EncounterState } from "./types";
import { millisecondsToElapsedFormat } from "./utils";

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
