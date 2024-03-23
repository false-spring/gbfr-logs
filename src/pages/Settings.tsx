import { Box, Checkbox, ColorInput, Fieldset, Select, Slider, Stack, Text, Tooltip } from "@mantine/core";
import { useTranslation } from "react-i18next";
import useSettings from "./useSettings";

const SettingsPage = () => {
  const { t, i18n } = useTranslation();
  const {
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
  } = useSettings();

  return (
    <Box>
      <Fieldset legend={t("ui.meter-settings")}>
        <Stack>
          <Select
            label={t("ui.language")}
            data={languages}
            defaultValue={i18n.language}
            allowDeselect={false}
            onChange={handleLanguageChange}
          />
          <ColorInput
            defaultValue={color_1}
            onChangeEnd={(value) => setMeterSettings({ color_1: value })}
            withEyeDropper={false}
            label={t("ui.player-1-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_2}
            onChangeEnd={(value) => setMeterSettings({ color_2: value })}
            withEyeDropper={false}
            label={t("ui.player-2-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_3}
            onChangeEnd={(value) => setMeterSettings({ color_3: value })}
            withEyeDropper={false}
            label={t("ui.player-3-color")}
            placeholder="Color"
          />
          <ColorInput
            defaultValue={color_4}
            onChangeEnd={(value) => setMeterSettings({ color_4: value })}
            withEyeDropper={false}
            label={t("ui.player-4-color")}
            placeholder="Color"
          />
          <Text size="sm">{t("ui.meter-transparency")}</Text>
          <Slider
            min={0}
            max={1}
            step={0.005}
            defaultValue={transparency}
            onChangeEnd={(value) => setMeterSettings({ transparency: value })}
          />
          <Checkbox
            label={t("ui.show-player-names")}
            checked={show_display_names}
            onChange={(event) => setMeterSettings({ show_display_names: event.currentTarget.checked })}
          />
          <Tooltip label={t("ui.streamer-mode-description")}>
            <Checkbox
              label={t("ui.streamer-mode")}
              checked={streamer_mode}
              onChange={(event) => setMeterSettings({ streamer_mode: event.currentTarget.checked })}
            />
          </Tooltip>
        </Stack>
      </Fieldset>
    </Box>
  );
};

export default SettingsPage;
