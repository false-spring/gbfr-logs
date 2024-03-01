import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import en from "./lang/en.json";
import zhCN from "./lang/zh-CN.json";

const resources = {
  en,
  "zh-CN": zhCN,
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    lng: "en",
    fallbackLng: "en",
    interpolation: {
      escapeValue: false,
    },
  });

declare global {
  interface Window {
    i18n: any;
  }
}

window.i18n = i18n;

export default i18n;
