import { useState, useEffect } from "react";
import { getVersion as tauriVersion } from "@tauri-apps/api/app";

export default function getVersion() {
  const [version, setVersion] = useState<string>("0.0.0");

  useEffect(() => {
    tauriVersion().then((v) => {
      setVersion(v);
    });
  });

  return {
    version,
  };
}
