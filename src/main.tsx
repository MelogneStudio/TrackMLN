import "@fontsource/khand/300.css";
import "@fontsource/khand/400.css";
import "@fontsource/khand/500.css";
import "@fontsource/khand/600.css";
import "@fontsource/khand/700.css";
import "@fontsource/dm-sans/400.css";
import "@fontsource/dm-sans/500.css";
import "@fontsource/dm-sans/700.css";
import "@fontsource/jetbrains-mono/400.css";

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/base.css";
import "./styles/shell.css";
import "./styles/home.css";
import "./styles/settings.css";
import "./styles/onboarding.css";
import "./styles/overlay.css";
import "./styles/alerts.css";

// No native right-click menu anywhere: it breaks the glass look and offers nothing useful.
window.addEventListener("contextmenu", (event) => event.preventDefault());

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
