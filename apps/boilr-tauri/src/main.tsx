import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

if (!("__TAURI_INTERNALS__" in window)) {
  await import("./devMock");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
