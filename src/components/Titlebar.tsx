import { appWindow } from "@tauri-apps/api/window";
import { Minus, Camera, ClipboardText } from "@phosphor-icons/react";
import { exportScreenshotToClipboard } from "../utils";

export const Titlebar = ({ onExportTextToClipboard }: { onExportTextToClipboard: () => void }) => {
  const onMinimize = () => {
    appWindow.minimize();
  };

  // @TODO(false): I've committed the sin of using divs as buttons. Replace later with actual buttons, please.
  return (
    <div data-tauri-drag-region className="titlebar transparent-bg font-sm">
      <div data-tauri-drag-region className="titlebar-left"></div>
      <div data-tauri-drag-region className="titlebar-right">
        <div className="titlebar-button" id="titlebar-text-export" onClick={onExportTextToClipboard}>
          <ClipboardText size={16} />
        </div>
        <div className="titlebar-button" id="titlebar-snapshot" onClick={exportScreenshotToClipboard}>
          <Camera size={16} />
        </div>
        <div className="titlebar-button" id="titlebar-minimize" onClick={onMinimize}>
          <Minus size={16} />
        </div>
      </div>
    </div>
  );
};
