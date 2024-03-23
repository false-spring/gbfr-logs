import { getVersion as tauriVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

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
