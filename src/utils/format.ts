const tryParseInt = (intString: string | number, defaultValue = 0) => {
  if (typeof intString === "number") {
    if (isNaN(intString)) return defaultValue;
    return intString;
  }

  let intNum;

  try {
    intNum = parseInt(intString);
    if (isNaN(intNum)) intNum = defaultValue;
  } catch {
    intNum = defaultValue;
  }

  return intNum;
};

/// Takes a number and returns a shortened version of it that is friendlier to read.
/// For example, 1200 would be returned as 1.2k, 1200000 as 1.2m, and so on.
export const humanizeNumbers = (n: number) => {
  if (n >= 1e3 && n < 1e6) return [+(n / 1e3).toFixed(1), "k"];
  if (n >= 1e6 && n < 1e9) return [+(n / 1e6).toFixed(1), "m"];
  if (n >= 1e9 && n < 1e12) return [+(n / 1e9).toFixed(1), "b"];
  if (n >= 1e12) return [+(n / 1e12).toFixed(1), "t"];
  else return [tryParseInt(n).toFixed(0), ""];
};

/// Takes a number of milliseconds and returns a string in the format of MM:SS.
export const millisecondsToElapsedFormat = (ms: number): string => {
  const date = new Date(Date.UTC(0, 0, 0, 0, 0, 0, ms));
  return `${date.getUTCMinutes().toString().padStart(2, "0")}:${date.getUTCSeconds().toString().padStart(2, "0")}`;
};

export const millisecondsToElapsedPreciseFormat = (ms: number): string => {
  const millis = Math.floor(Math.max(ms, 0) % 1000);
  return `${millisecondsToElapsedFormat(ms)}.${millis.toString().padStart(3, "0")}`;
};

export const epochToLocalTime = (epoch: number): string => {
  const utc = new Date(epoch);

  return new Intl.DateTimeFormat("default", {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
  }).format(utc);
};

export const compareGameVersions = (a: string, b: string): number => {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  const len = Math.max(pa.length, pb.length);

  for (let i = 0; i < len; i++) {
    const da = pa[i] ?? 0;
    const db = pb[i] ?? 0;
    if (da !== db) return da - db;
  }

  return 0;
};

/// Converts a number to a hexadecimal string.
export const toHash = (num: number): string => num.toString(16);

/// Converts a number to a hexadecimal string and pads it to 8 characters.
export const toHashString = (num: number | undefined): string => (num ? num.toString(16).padStart(8, "0") : "");
