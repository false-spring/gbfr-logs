import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import { resolveResource } from "@tauri-apps/api/path";
import { readTextFile } from "@tauri-apps/api/fs";

const loadLanguageFromPath = async (language: string) => {
  const resourcePath = await resolveResource(`lang/${language}.json`);
  return JSON.parse(await readTextFile(resourcePath));
};

const en = await loadLanguageFromPath("en");
const zhCN = await loadLanguageFromPath("zh-CN");
const zhTW = await loadLanguageFromPath("zh-TW");
const koKR = await loadLanguageFromPath("ko-KR");

const resources = {
  en,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  "ko-KR": koKR,
};

export const SUPPORTED_LANGUAGES: { [key: string]: string } = {
  en: "English",
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  "ko-KR": "한국어",
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: ["en", "zh-CN", "zh-TW"],
    interpolation: {
      escapeValue: false,
    },
    react: {
      bindI18nStore: "added",
    },
  });

declare global {
  interface Window {
    /* eslint-disable */
    i18n: any;
  }
}

window.i18n = i18n;

export default i18n;
