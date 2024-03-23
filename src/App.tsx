import { BrowserRouter, Routes, Route } from "react-router-dom";

import { Meter } from "./pages/Meter";

import Logs from "./pages/Logs";
import SettingsPage from "./pages/Settings";
import { ViewPage as LogViewPage } from "./pages/logs/View";
import { IndexPage as LogIndexPage } from "./pages/logs/Index";

import "./App.css";

export const App = () => {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Meter />} />
        <Route path="/logs" element={<Logs />}>
          <Route index element={<LogIndexPage />} />
          <Route path=":id" element={<LogViewPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
};
