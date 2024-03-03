import { BrowserRouter, Routes, Route } from "react-router-dom";

import Meter from "./pages/Meter";
import Logs, { LogIndexPage, LogViewPage, SettingsPage } from "./pages/Logs";

import "./App.css";

const App = () => {
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

export default App;
