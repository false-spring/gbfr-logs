import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./lang/en.json";

const resources = {
  en,
};

console.log(resources);
i18n.use(initReactI18next).init({
  resources,
  lng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
