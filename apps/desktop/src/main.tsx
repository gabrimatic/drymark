import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App, type AppView } from "./App";
import { createDesktopBridge } from "./bridge";

const requested = new URLSearchParams(window.location.search).get("view");
const view: AppView = requested === "settings" || requested === "toast" ? requested : "tray";
const root = document.getElementById("root");

if (root === null) throw new Error("DryMark root element is missing");

createRoot(root).render(
  <StrictMode>
    <App view={view} bridge={createDesktopBridge()} />
  </StrictMode>,
);
