import html2canvas from "html2canvas";
import toast from "react-hot-toast";

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

// Takes a number and returns a shortened version of it that is friendlier to read.
// For example, 1200 would be returned as 1.2k, 1200000 as 1.2m, and so on.
export const humanizeNumbers = (n: number) => {
  if (n >= 1e3 && n < 1e6) return [+(n / 1e3).toFixed(1), "k"];
  if (n >= 1e6 && n < 1e9) return [+(n / 1e6).toFixed(1), "m"];
  if (n >= 1e9 && n < 1e12) return [+(n / 1e9).toFixed(1), "b"];
  if (n >= 1e12) return [+(n / 1e12).toFixed(1), "t"];
  else return [tryParseInt(n).toFixed(0), ""];
};

export const millisecondsToElapsedFormat = (ms: number): string => {
  const date = new Date(Date.UTC(0, 0, 0, 0, 0, 0, ms));
  return `${date.getUTCMinutes().toString().padStart(2, "0")}:${date
    .getUTCSeconds()
    .toString()
    .padStart(2, "0")}`;
};

export const exportScreenshotToClipboard = () => {
  const app = document.querySelector(".app") as HTMLElement;

  html2canvas(app, {
    backgroundColor: "transparent",
  }).then((canvas) => {
    canvas.toBlob((blob) => {
      if (blob) {
        const item = new ClipboardItem({ "image/png": blob });
        navigator.clipboard.write([item]);
        toast.success("Screenshot copied to clipboard!");
      }
    });
  });
};
