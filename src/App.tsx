import { BrowserRouter, Routes, Route, Router } from "react-router-dom";

import Meter from "./pages/Meter";
import Logs from "./pages/Logs";

import "./App.css";

const App = () => {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Meter />} />
        <Route path="/logs" element={<Logs />} />
      </Routes>
    </BrowserRouter>
  );
};

export default App;
