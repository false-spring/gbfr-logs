import { Fragment } from "react";
import { EncounterState } from "../types";
import { humanizeNumbers, millisecondsToElapsedFormat } from "../utils";

const TeamDamageStats = ({ encounterState }: { encounterState: EncounterState }) => {
  const [teamDps, dpsUnit] = humanizeNumbers(encounterState.dps);
  const [totalTeamDmg, dmgUnit] = humanizeNumbers(encounterState.totalDamage);

  return (
    <Fragment>
      <div className="encounter-totalDamage item">
        {totalTeamDmg}
        <span className="unit font-sm">{dmgUnit} -</span>
      </div>
      <div className="encounter-totalDps item">
        {teamDps}
        <span className="unit font-sm">{dpsUnit}/s -</span>
      </div>
    </Fragment>
  );
};

const EncounterStatus = ({ encounterState, elapsedTime }: { encounterState: EncounterState; elapsedTime: number }) => {
  if (encounterState.status === "Waiting") {
    return <div className="encounter-status">{encounterState.status}..</div>;
  } else if (encounterState.status === "InProgress") {
    return (
      <Fragment>
        <div className="encounter-elapsedTime item">{millisecondsToElapsedFormat(elapsedTime)}</div>
      </Fragment>
    );
  } else if (encounterState.status === "Stopped") {
    return (
      <Fragment>
        <div className="encounter-elapsedTime">
          {millisecondsToElapsedFormat(encounterState.endTime - encounterState.startTime)}
        </div>
      </Fragment>
    );
  }
};

export const Footer = ({ encounterState, elapsedTime }: { encounterState: EncounterState; elapsedTime: number }) => {
  return (
    <div className="footer transparent-bg font-sm">
      <div className="version">
        GBFR Logs <span className="version-number">0.0.3</span>
      </div>
      {encounterState.totalDamage > 0 && <TeamDamageStats encounterState={encounterState} />}
      <EncounterStatus encounterState={encounterState} elapsedTime={elapsedTime} />
    </div>
  );
};
