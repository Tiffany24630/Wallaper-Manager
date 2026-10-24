import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import "./styles/index.css";

const isSmoke = (window as Window & { __LUMINA_SMOKE__?: boolean }).__LUMINA_SMOKE__ === true;

if (isSmoke) {
  void invoke("record_smoke_diagnostic", {
    stage: "frontend_entry",
    detail: null,
  }).catch(() => undefined);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
