import { DragDropContext, Draggable, Droppable } from "@hello-pangea/dnd";
import {
  ActionIcon,
  Box,
  Button,
  Checkbox,
  ColorInput,
  Divider,
  Fieldset,
  Flex,
  Menu,
  Select,
  Slider,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { DotsSixVertical } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import useSettings from "./useSettings";

const SettingsPage = () => {
  const { t, i18n } = useTranslation();
  const [debugMode, setDebugMode] = useState(false);

  const {
    color_1,
    color_2,
    color_3,
    color_4,
    transparency,
    show_display_names,
    streamer_mode,
    show_full_values,
    use_condensed_skills,
    setMeterSettings,
    languages,
    handleLanguageChange,
    overlay_columns,
    handleReorderOverlayColumns,
    availableOverlayColumns,
    addOverlayColumn,
    removeOverlayColumn,
  } = useSettings();

  const toggleDebugMode = () => {
    const enabled = !debugMode;
    setDebugMode(enabled);
    invoke("set_debug_mode", { enabled });
    console.info("Debug Mode:", enabled ? "Enabled" : "Disabled");
  };

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
          <Tooltip label={t("ui.show-full-values-description")}>
            <Checkbox
              label={t("ui.show-full-values")}
              checked={show_full_values}
              onChange={(event) => setMeterSettings({ show_full_values: event.currentTarget.checked })}
            />
          </Tooltip>
          <Tooltip label={t("ui.use-condensed-skills-description")}>
            <Checkbox
              label={t("ui.use-condensed-skills")}
              checked={use_condensed_skills}
              onChange={(event) => setMeterSettings({ use_condensed_skills: event.currentTarget.checked })}
            />
          </Tooltip>
          <Tooltip label={t("ui.debug-mode-description")}>
            <Checkbox label={t("ui.debug-mode")} checked={debugMode} onChange={toggleDebugMode} />
          </Tooltip>
          <Divider />
          <Text size="sm">Customize Overlay Meter Columns</Text>
          <Menu shadow="md" trigger="hover" openDelay={100} closeDelay={400}>
            <Menu.Target>
              <Button>Add column</Button>
            </Menu.Target>
            <Menu.Dropdown>
              {availableOverlayColumns.map((item) => (
                <Menu.Item key={item} onClick={() => addOverlayColumn(item)}>
                  {t(`ui.meter-columns.${item}`)} - {t(`ui.meter-columns.${item}-description`)}
                </Menu.Item>
              ))}
            </Menu.Dropdown>
          </Menu>
          <DragDropContext onDragEnd={handleReorderOverlayColumns}>
            <Droppable droppableId="overlay-columns">
              {(droppableProvided) => (
                <Stack ref={droppableProvided.innerRef}>
                  {overlay_columns.map((item, index) => (
                    <Draggable key={item} draggableId={item} index={index}>
                      {(draggableProvided) => (
                        <Box
                          bg="var(--mantine-color-dark-8)"
                          display="flex"
                          p={10}
                          ref={draggableProvided.innerRef}
                          {...draggableProvided.draggableProps}
                          {...draggableProvided.dragHandleProps}
                        >
                          <Flex align="center" flex={1}>
                            <DotsSixVertical size={16} style={{ cursor: "grab", marginRight: "0.5em" }} />
                            {t(`ui.meter-columns.${item}`)} - {t(`ui.meter-columns.${item}-description`)}
                          </Flex>
                          <Flex align="center">
                            <ActionIcon
                              aria-label="Remove column"
                              variant="transparent"
                              color="gray"
                              onClick={() => removeOverlayColumn(item)}
                            >
                              x
                            </ActionIcon>
                          </Flex>
                        </Box>
                      )}
                    </Draggable>
                  ))}
                  {droppableProvided.placeholder}
                </Stack>
              )}
            </Droppable>
          </DragDropContext>
        </Stack>
      </Fieldset>
    </Box>
  );
};

export default SettingsPage;
