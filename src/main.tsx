import "@fontsource-variable/noto-sans";
import "@mantine/charts/styles.css";
import { Autocomplete, MantineProvider, MultiSelect, Select, TagsInput, createTheme, rem } from "@mantine/core";
import "@mantine/core/styles.css";
import ReactDOM from "react-dom/client";
import "./styles.css";

import { ModalsProvider } from "@mantine/modals";
import { App } from "./App";

// Mantine dropdown ScrollAreas default to type="hover", hiding the cue that a long list scrolls.
const alwaysVisibleDropdownScrollbar = {
  defaultProps: { scrollAreaProps: { type: "always" as const } },
};

const theme = createTheme({
  fontFamily: '"Noto Sans Variable", Inter, Avenir, Helvetica, Arial, sans-serif',
  fontSizes: {
    xs: rem(14),
    sm: "12",
    md: "14",
    lg: "16",
    xl: "18",
  },
  components: {
    Select: Select.extend(alwaysVisibleDropdownScrollbar),
    MultiSelect: MultiSelect.extend(alwaysVisibleDropdownScrollbar),
    Autocomplete: Autocomplete.extend(alwaysVisibleDropdownScrollbar),
    TagsInput: TagsInput.extend(alwaysVisibleDropdownScrollbar),
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <MantineProvider theme={theme} defaultColorScheme="dark">
    <ModalsProvider>
      <App />
    </ModalsProvider>
  </MantineProvider>
);
