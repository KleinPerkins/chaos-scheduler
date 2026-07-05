import MenuBarPopup from "./components/MenuBarPopup";
import Dashboard from "./components/Dashboard";
import ErrorBoundary from "./components/ErrorBoundary";

function getView(): "popup" | "dashboard" {
  const params = new URLSearchParams(window.location.search);
  return params.get("view") === "popup" ? "popup" : "dashboard";
}

export default function App() {
  const view = getView();

  if (view === "popup") {
    return (
      <ErrorBoundary viewName="Menu bar popup">
        <MenuBarPopup />
      </ErrorBoundary>
    );
  }

  return (
    <ErrorBoundary viewName="Dashboard">
      <Dashboard />
    </ErrorBoundary>
  );
}
