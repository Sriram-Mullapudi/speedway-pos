import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import CustomerDisplay from "./screens/CustomerDisplay";
import "./styles.css";

// A dedicated presentation surface (customer-facing display) loads when the
// window is opened with ?display=customer — it never mounts the cashier UI.
const isCustomerDisplay = new URLSearchParams(window.location.search).get("display") === "customer";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isCustomerDisplay ? <CustomerDisplay /> : <App />}
  </React.StrictMode>
);
