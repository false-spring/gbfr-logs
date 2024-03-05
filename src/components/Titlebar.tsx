import { appWindow } from "@tauri-apps/api/window";
import { Minus, Camera, ClipboardText } from "@phosphor-icons/react";
import { exportScreenshotToClipboard, humanizeNumbers, millisecondsToElapsedFormat } from "../utils";
import { Tooltip } from "@mantine/core";
import { EncounterState } from "../types";
import { Fragment } from "react";

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
        <span className="unit font-sm">{dpsUnit}/s</span>
      </div>
    </Fragment>
  );
};

const EncounterStatus = ({ encounterState, elapsedTime }: { encounterState: EncounterState; elapsedTime: number }) => {
  if (encounterState.status === "Waiting") {
    return <div className="encounter-status item">{encounterState.status}..</div>;
  } else if (encounterState.status === "InProgress") {
    return (
      <Fragment>
        <div className="encounter-elapsedTime item">{millisecondsToElapsedFormat(elapsedTime)}</div>
      </Fragment>
    );
  } else if (encounterState.status === "Stopped") {
    return (
      <Fragment>
        <div className="encounter-elapsedTime item">
          {millisecondsToElapsedFormat(encounterState.endTime - encounterState.startTime)}
        </div>
      </Fragment>
    );
  }
};

export const Titlebar = ({
  onExportTextToClipboard,
  encounterState,
  elapsedTime,
}: {
  onExportTextToClipboard: () => void;
  encounterState: EncounterState;
  elapsedTime: number;
}) => {
  const onMinimize = () => {
    appWindow.minimize();
  };

  // @TODO(false): I've committed the sin of using divs as buttons. Replace later with actual buttons, please.
  return (
    <div data-tauri-drag-region className="titlebar transparent-bg font-sm">
      <div data-tauri-drag-region className="titlebar-left">
        <div className="version">
          GBFR Logs <span className="version-number">0.0.4</span> -
        </div>
        {encounterState.totalDamage > 0 && <TeamDamageStats encounterState={encounterState} />}
      </div>
      <div data-tauri-drag-region className="titlebar-right">
        <EncounterStatus encounterState={encounterState} elapsedTime={elapsedTime} />
        <Tooltip label="Copy text to clipboard" color="dark">
          <div className="titlebar-button" id="titlebar-text-export" onClick={onExportTextToClipboard}>
            <ClipboardText size={16} />
          </div>
        </Tooltip>
        <Tooltip label="Copy screenshot to clipboard" color="dark">
          <div className="titlebar-button" id="titlebar-snapshot" onClick={exportScreenshotToClipboard}>
            <Camera size={16} />
          </div>
        </Tooltip>
        <div className="titlebar-button" id="titlebar-minimize" onClick={onMinimize}>
          <Minus size={16} />
        </div>
      </div>
    </div>
  );
};
