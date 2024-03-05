import ReactDOM from "react-dom/client";
import { createTheme, MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "@mantine/charts/styles.css";
import "./styles.css";
import "@fontsource-variable/noto-sans";

import { App } from "./App";

const theme = createTheme({
  fontFamily: '"Noto Sans Variable", Inter, Avenir, Helvetica, Arial, sans-serif',
  fontSizes: {
    xs: "10",
    sm: "12",
    md: "14",
    lg: "16",
    xl: "18",
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <MantineProvider theme={theme} defaultColorScheme="dark">
    <App />
  </MantineProvider>
);
