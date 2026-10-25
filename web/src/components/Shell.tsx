import { useCallback, useEffect, useRef, useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router-dom";
import {
  Crosshair,
  Gauge,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { errorMessage, getHealth, isAbortError } from "../api";
import { cn } from "../utils";

const NAV: Array<{ to: string; label: string; index: string; icon: LucideIcon }> = [
  { to: "/workspace", label: "Workspace", index: "01", icon: Crosshair },
  { to: "/benchmark", label: "Benchmark Lab", index: "02", icon: Gauge },
  { to: "/rules", label: "Rules", index: "03", icon: ShieldCheck },
  { to: "/settings", label: "Settings", index: "04", icon: SlidersHorizontal },
];

const TITLES: Record<string, string> = {
  "/workspace": "Workspace",
  "/benchmark": "Benchmark Lab",
  "/rules": "Rules",
  "/settings": "Settings",
};

type HealthState =
  | { state: "checking" }
  | { state: "online"; version: string; apiVersion: number }
  | { state: "offline"; message: string };

function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <Link
      to="/workspace"
      className="group inline-flex items-center gap-3 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-rust"
      aria-label="ContextDrop workspace"
    >
      <span
        aria-hidden="true"
        className={cn(
          "relative grid shrink-0 place-items-center overflow-hidden border border-ink bg-ink font-display text-paper",
          compact ? "h-8 w-8 text-lg" : "h-10 w-10 text-2xl",
        )}
      >
        C
        <span className="absolute bottom-0 right-0 h-2 w-2 bg-rust" />
      </span>
      <span className="leading-none">
        <span className={cn("block font-display tracking-[-0.02em]", compact ? "text-[22px]" : "text-[26px]")}>
          Context<span className="italic text-rust">Drop</span>
        </span>
        {!compact ? (
          <span className="mt-1.5 block font-mono text-[8.5px] uppercase tracking-[0.18em] text-muted">
            Deterministic local compression
          </span>
        ) : null}
      </span>
    </Link>
  );
}

function HealthIndicator({
  health,
  onRetry,
  compact = false,
}: {
  health: HealthState;
  onRetry: () => void;
  compact?: boolean;
}) {
  if (health.state === "checking") {
    return (
      <span className="inline-flex items-center gap-2 font-mono text-[9px] uppercase tracking-[0.12em] text-muted">
        <span aria-hidden="true" className="h-1.5 w-1.5 animate-pulse rounded-full bg-amber" />
        Checking API
      </span>
    );
  }
  if (health.state === "online") {
    return (
      <span
        className="inline-flex items-center gap-2 font-mono text-[9px] uppercase tracking-[0.12em] text-moss"
        title={`Local API ${health.apiVersion} · engine ${health.version}`}
      >
        <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-moss" />
        {compact ? "Local online" : `Local API · v${health.version}`}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-[9px] uppercase tracking-[0.1em] text-danger">
      <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-danger" />
      API offline
      <button
        type="button"
        onClick={onRetry}
        aria-label="Retry local API connection"
        className="ml-0.5 inline-flex h-6 w-6 items-center justify-center rounded-[2px] text-ink-soft hover:bg-paper-deep hover:text-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-rust"
        title={health.message}
      >
        <RefreshCw aria-hidden="true" className="h-3 w-3" />
      </button>
    </span>
  );
}

export function AppShell() {
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const [health, setHealth] = useState<HealthState>({ state: "checking" });
  const [healthAttempt, setHealthAttempt] = useState(0);

  const retryHealth = useCallback(() => setHealthAttempt((value) => value + 1), []);

  useEffect(() => {
    const controller = new AbortController();
    setHealth({ state: "checking" });
    getHealth(controller.signal)
      .then((response) => {
        setHealth({
          state: "online",
          version: response.engine_version,
          apiVersion: response.api_version,
        });
      })
      .catch((error: unknown) => {
        if (!isAbortError(error)) {
          setHealth({ state: "offline", message: errorMessage(error) });
        }
      });
    return () => controller.abort();
  }, [healthAttempt]);

  useEffect(() => {
    const title = TITLES[location.pathname] ?? "Not found";
    document.title = `${title} · ContextDrop`;
    window.scrollTo({ top: 0, behavior: "instant" });
    const frame = window.requestAnimationFrame(() => {
      mainRef.current?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [location.pathname]);

  return (
    <div className="min-h-screen bg-paper text-ink">
      <a
        href="#main-content"
        className="fixed left-3 top-3 z-[80] -translate-y-20 bg-ink px-4 py-2 text-sm font-medium text-paper transition-transform focus:translate-y-0"
      >
        Skip to content
      </a>

      <aside className="fixed inset-y-0 left-0 z-40 hidden w-[248px] flex-col border-r border-line bg-paper-deep/35 px-6 pb-6 pt-7 lg:flex">
        <Brand />
        <p className="mt-8 max-w-[180px] text-[13px] leading-5 text-ink-soft">
          Remove deterministic repetition. Keep the diagnostic evidence.
        </p>

        <nav aria-label="Primary" className="mt-9 flex flex-col gap-1">
          {NAV.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    "group flex min-h-11 items-center gap-3 rounded-[2px] px-2.5 text-[13px] transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust",
                    isActive
                      ? "bg-ink font-medium text-paper"
                      : "text-ink-soft hover:bg-paper-deep hover:text-ink",
                  )
                }
              >
                {({ isActive }) => (
                  <>
                    <span
                      className={cn(
                        "w-5 font-mono text-[9px] tracking-[0.08em]",
                        isActive ? "text-rust-soft" : "text-muted",
                      )}
                    >
                      {item.index}
                    </span>
                    <Icon aria-hidden="true" className="h-4 w-4" strokeWidth={1.6} />
                    <span>{item.label}</span>
                  </>
                )}
              </NavLink>
            );
          })}
        </nav>

        <div className="mt-auto border-t border-line pt-5">
          <div className="mb-3 flex items-center justify-between gap-2">
            <span className="font-mono text-[9px] uppercase tracking-[0.14em] text-muted">
              Runtime
            </span>
            <HealthIndicator health={health} onRetry={retryHealth} />
          </div>
          <p className="font-mono text-[9px] leading-5 tracking-[0.03em] text-muted">
            Loopback only · No telemetry
            <br />
            No model calls · No uploads
          </p>
        </div>
      </aside>

      <header className="sticky top-0 z-30 border-b border-line bg-paper/95 px-4 backdrop-blur-sm sm:px-6 lg:hidden">
        <div className="flex h-16 items-center justify-between gap-4">
          <Brand compact />
          <HealthIndicator health={health} onRetry={retryHealth} compact />
        </div>
      </header>

      <main
        id="main-content"
        ref={mainRef}
        tabIndex={-1}
        className="min-w-0 pb-24 outline-none lg:pb-0 lg:pl-[248px]"
      >
        <div className="mx-auto max-w-[1480px] px-4 py-7 sm:px-6 lg:px-10 lg:py-11 xl:px-14">
          <div key={location.pathname} className="page-enter">
            <Outlet />
          </div>
        </div>
      </main>

      <nav
        aria-label="Primary mobile navigation"
        className="fixed inset-x-0 bottom-0 z-40 border-t border-line bg-paper/95 pb-[env(safe-area-inset-bottom)] backdrop-blur-sm lg:hidden"
      >
        <div className="grid h-[68px] grid-cols-4">
          {NAV.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    "relative flex min-h-[44px] flex-col items-center justify-center gap-1 text-[10px] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-rust",
                    isActive ? "text-rust" : "text-ink-soft",
                  )
                }
              >
                {({ isActive }) => (
                  <>
                    {isActive ? (
                      <span aria-hidden="true" className="absolute left-1/2 top-0 h-0.5 w-8 -translate-x-1/2 bg-rust" />
                    ) : null}
                    <Icon aria-hidden="true" className="h-[18px] w-[18px]" strokeWidth={1.7} />
                    <span>{item.label === "Benchmark Lab" ? "Benchmark" : item.label}</span>
                  </>
                )}
              </NavLink>
            );
          })}
        </div>
      </nav>
    </div>
  );
}
