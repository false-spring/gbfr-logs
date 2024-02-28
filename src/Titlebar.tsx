import { appWindow } from "@tauri-apps/api/window";
import { Minus, Camera } from "@phosphor-icons/react";
import { exportScreenshotToClipboard } from "./utils";

export const Titlebar = () => {
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
