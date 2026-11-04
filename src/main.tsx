import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import QuickWindow from "./app/QuickWindow";
import "./styles/globals.css";

const isQuick = new URLSearchParams(window.location.search).get("quick") === "1";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  isQuick ? (
    <QuickWindow />
  ) : (
    <React.StrictMode>
      <App />
    </React.StrictMode>
  ),
);
