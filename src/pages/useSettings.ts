import { useMeterSettingsStore } from "@/Store";
import { SUPPORTED_LANGUAGES } from "@/i18n";
import { useTranslation } from "react-i18next";

export default function useSettings() {
  const { color_1, color_2, color_3, color_4, transparency, show_display_names, streamer_mode, setMeterSettings } =
    useMeterSettingsStore((state) => ({
      color_1: state.color_1,
      color_2: state.color_2,
      color_3: state.color_3,
      color_4: state.color_4,
      transparency: state.transparency,
      show_display_names: state.show_display_names,
      streamer_mode: state.streamer_mode,
      setMeterSettings: state.set,
    }));

  const { i18n } = useTranslation();

  const languages = Object.keys(SUPPORTED_LANGUAGES).map((key) => ({ value: key, label: SUPPORTED_LANGUAGES[key] }));

  const handleLanguageChange = (language: string | null) => {
    i18n.changeLanguage(language as string);
  };

  return {
    color_1,
    color_2,
    color_3,
    color_4,
    transparency,
    show_display_names,
    streamer_mode,
    setMeterSettings,
    languages,
    handleLanguageChange,
  };
}
