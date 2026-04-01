import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import i18n from "./lib/i18n";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Suspense fallback={<div>{i18n.t("common:loading")}</div>}>
      <App />
    </Suspense>
  </React.StrictMode>,
);
