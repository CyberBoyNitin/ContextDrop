import { Component, type ErrorInfo, type ReactNode } from "react";
import { BrowserRouter, Navigate, Route, Routes, Link } from "react-router-dom";
import { AppShell } from "./components/Shell";
import { Button } from "./components/ui";
import { SettingsProvider } from "./settings";
import { BenchmarkPage } from "./pages/BenchmarkPage";
import { RulesPage } from "./pages/RulesPage";
import { SettingsPage } from "./pages/SettingsPage";
import { WorkspacePage } from "./pages/WorkspacePage";

class AppErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(_error: Error, _info: ErrorInfo) {
    // Intentionally do not log error details: they can contain pasted input in
    // rare rendering failures. The dashboard offers a clean recovery state.
  }

  render() {
    if (this.state.failed) {
      return (
        <main className="grid min-h-screen place-items-center bg-paper px-6 text-ink">
          <div className="max-w-md border border-line bg-surface p-8">
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-rust">
              Dashboard error
            </p>
            <h1 className="mt-3 font-display text-4xl leading-none">A local rendering error occurred.</h1>
            <p className="mt-4 text-sm leading-6 text-ink-soft">
              No data was sent anywhere. Reload the dashboard to return to a clean state. Any
              processing result can be regenerated locally.
            </p>
            <Button className="mt-6" variant="primary" onClick={() => window.location.reload()}>
              Reload dashboard
            </Button>
          </div>
        </main>
      );
    }
    return this.props.children;
  }
}

function NotFoundPage() {
  return (
    <div className="mx-auto flex min-h-[60vh] max-w-xl flex-col justify-center">
      <p className="font-mono text-[10px] uppercase tracking-[0.15em] text-rust">404 / Not found</p>
      <h1 className="mt-3 font-display text-6xl leading-[0.9]">This route has no context.</h1>
      <p className="mt-5 max-w-md text-[15px] leading-6 text-ink-soft">
        The dashboard only exposes four local views: Workspace, Benchmark Lab, Rules, and Settings.
      </p>
      <Link
        to="/workspace"
        className="mt-7 inline-flex w-fit items-center border border-ink bg-ink px-4 py-2.5 text-sm font-medium text-paper transition-colors hover:border-rust hover:bg-rust focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
      >
        Return to Workspace
      </Link>
    </div>
  );
}

export default function App() {
  return (
    <AppErrorBoundary>
      <SettingsProvider>
        <BrowserRouter>
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<Navigate to="/workspace" replace />} />
              <Route path="/workspace" element={<WorkspacePage />} />
              <Route path="/benchmark" element={<BenchmarkPage />} />
              <Route path="/rules" element={<RulesPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<NotFoundPage />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </SettingsProvider>
    </AppErrorBoundary>
  );
}
