import MenuBarPopup from "./components/MenuBarPopup";
import Dashboard from "./components/Dashboard";

function getView(): "popup" | "dashboard" {
  const params = new URLSearchParams(window.location.search);
  return params.get("view") === "popup" ? "popup" : "dashboard";
}

export default function App() {
  const view = getView();

  if (view === "popup") {
    return <MenuBarPopup />;
  }

  return <Dashboard />;
}
