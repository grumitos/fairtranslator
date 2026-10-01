import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./lib/i18n";
import "./index.css";

// Sin respaldo visible: mientras carga, sigue a la vista la pantalla de inicio de index.html.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Suspense fallback={null}>
      <App />
    </Suspense>
  </React.StrictMode>,
);
