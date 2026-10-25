import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  Check,
  DollarSign,
  Download,
  HardDrive,
  RotateCcw,
  Server,
  ShieldCheck,
  SlidersHorizontal,
  TriangleAlert,
} from "lucide-react";
import { errorMessage, getHealth, isAbortError } from "../api";
import type { HealthResponse } from "../types";
import {
  DEFAULT_SETTINGS,
  MAX_INPUT_OPTIONS,
  SERVER_MAX_INPUT_BYTES,
  useSettings,
  validateSettings,
  type Settings,
} from "../settings";
import { cn, downloadText, formatBytes, formatNumber, pluralize } from "../utils";
import { StrategyPicker } from "../components/StrategyPicker";
import {
  Badge,
  Button,
  ErrorNotice,
  PageHeader,
  Select,
  Toggle,
} from "../components/ui";

type HealthState =
  | { state: "loading" }
  | { state: "online"; data: HealthResponse }
  | { state: "offline"; message: string };

function SettingsSection({
  index,
  title,
  description,
  children,
}: {
  index: string;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="border border-line bg-surface">
      <div className="flex items-start gap-3 border-b border-line p-4 sm:p-5">
        <span className="mt-0.5 font-mono text-[9px] tracking-[0.12em] text-rust">{index}</span>
        <div>
          <h2 className="text-[15px] font-semibold tracking-[-0.01em]">{title}</h2>
          <p className="mt-1 max-w-2xl text-[12.5px] leading-5 text-muted">{description}</p>
        </div>
      </div>
      <div className="p-4 sm:p-5">{children}</div>
    </section>
  );
}

export function SettingsPage() {
  const { settings, saveSettings, resetSettings, persisted } = useSettings();
  const [draft, setDraft] = useState<Settings>(settings);
  const [confirmReset, setConfirmReset] = useState(false);
  const [saveStatus, setSaveStatus] = useState<string | null>(null);
  const [health, setHealth] = useState<HealthState>({ state: "loading" });
  const [healthAttempt, setHealthAttempt] = useState(0);
  const healthAbort = useRef<AbortController | null>(null);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  useEffect(() => {
    healthAbort.current?.abort();
    const controller = new AbortController();
    healthAbort.current = controller;
    setHealth({ state: "loading" });
    getHealth(controller.signal)
      .then((data) => setHealth({ state: "online", data }))
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setHealth({ state: "offline", message: errorMessage(caught) });
        }
      })
      .finally(() => {
        if (healthAbort.current === controller) {
          healthAbort.current = null;
        }
      });
    return () => controller.abort();
  }, [healthAttempt]);

  useEffect(() => {
    return () => healthAbort.current?.abort();
  }, []);

  const normalizedDraft = { ...draft, currency: draft.currency.trim().toUpperCase() };
  const errors = validateSettings(normalizedDraft);
  const dirty = JSON.stringify(normalizedDraft) !== JSON.stringify(settings);
  const canSave = dirty && errors.length === 0;

  function updateDraft(patch: Partial<Settings>) {
    setDraft((current) => ({ ...current, ...patch }));
    setSaveStatus(null);
  }

  function save() {
    if (!canSave) return;
    saveSettings(normalizedDraft);
    setDraft(normalizedDraft);
    setSaveStatus(
      `Saved locally at ${new Date().toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })}. New runs will use these defaults.`,
    );
  }

  function discard() {
    setDraft(settings);
    setSaveStatus("Unsaved changes discarded.");
    setConfirmReset(false);
  }

  function restoreDefaults() {
    resetSettings();
    setDraft({ ...DEFAULT_SETTINGS, preservePatterns: [] });
    setConfirmReset(false);
    setSaveStatus("Defaults restored. Custom preserve patterns were cleared.");
  }

  function exportSettings() {
    downloadText(
      "contextdrop-settings.json",
      JSON.stringify(
        {
          schema: "contextdrop.settings.v1",
          exported_at: new Date().toISOString(),
          settings: normalizedDraft,
        },
        null,
        2,
      ),
      "application/json;charset=utf-8",
    );
  }

  return (
    <>
      <PageHeader
        index="04 / Settings"
        title="Settings"
        description="Choose local defaults for compression, token measurement, and preservation. These values stay in this browser and are applied to new runs."
        actions={
          <>
            <Button variant="secondary" size="md" onClick={discard} disabled={!dirty}>
              Discard
            </Button>
            <Button variant="primary" size="md" onClick={save} disabled={!canSave}>
              <Check aria-hidden="true" className="h-4 w-4" />
              Save changes
            </Button>
          </>
        }
        meta={
          <>
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Schema <span className="text-ink">v1</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Storage{" "}
              <span className={persisted ? "text-moss" : "text-danger"}>
                {persisted ? "browser localStorage" : "session only"}
              </span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Patterns <span className="text-ink">{formatNumber(settings.preservePatterns.length)}</span>
            </span>
            <span className={cn("ml-auto font-mono text-[9px] uppercase tracking-[0.1em]", dirty ? "text-amber-dark" : "text-muted")}>
              {dirty ? "unsaved changes" : "saved defaults"}
            </span>
          </>
        }
      />

      <div className="sr-only" role="status" aria-live="polite">
        {saveStatus ?? ""}
      </div>
      {saveStatus ? (
        <div className="mb-4 flex items-start gap-2 border border-moss/25 bg-moss-soft/45 px-3.5 py-2.5 text-[12px] leading-5 text-moss">
          <Check aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{saveStatus}</span>
        </div>
      ) : null}
      {errors.length > 0 ? (
        <div
          role="alert"
          className="mb-4 border border-danger/30 bg-danger-soft/55 px-3.5 py-3 text-[12px] leading-5 text-danger"
        >
          <p className="font-medium">Fix these before saving:</p>
          <ul className="mt-1 list-disc space-y-0.5 pl-5">
            {errors.map((error) => (
              <li key={error}>{error}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_320px]">
        <div className="space-y-4">
          <SettingsSection
            index="01"
            title="Compression defaults"
            description="The Workspace starts with these values. Each run can still override them from its Options panel."
          >
            <StrategyPicker
              value={draft.strategy}
              onChange={(strategy) => updateDraft({ strategy })}
              showDescriptions
              name="default-strategy"
            />
            <p className="mt-2 text-[11px] leading-5 text-muted">
              Changing the default strategy does not overwrite any existing result. Run again with
              the Workspace strategy picker to apply it.
            </p>

            <div className="mt-5 grid gap-4 border-t border-line pt-5 sm:grid-cols-2">
              <Select
                label="Default token measurement"
                value={draft.tokenizer}
                onChange={(event) =>
                  updateDraft({ tokenizer: event.target.value as Settings["tokenizer"] })
                }
                hint="Exact tokenizers run locally. Initialization may take a moment on the first run."
              >
                <option value="o200k_base">o200k_base · exact</option>
                <option value="cl100k_base">cl100k_base · exact</option>
                <option value="estimate">Heuristic estimate</option>
              </Select>
              <Select
                label="Default input format"
                value={draft.inputFormat}
                onChange={(event) =>
                  updateDraft({ inputFormat: event.target.value as Settings["inputFormat"] })
                }
                hint="Auto-detection is conservative and can be overridden per run."
              >
                <option value="auto">Auto-detect</option>
                <option value="plain-text">Plain text</option>
                <option value="terminal">Terminal</option>
                <option value="build-log">Build log</option>
                <option value="test-output">Test output</option>
                <option value="json">JSON</option>
                <option value="structured-log">Structured log</option>
              </Select>
              <Select
                label="Maximum input size"
                value={draft.maxInputBytes}
                onChange={(event) => updateDraft({ maxInputBytes: Number(event.target.value) })}
                hint={`The local API rejects inputs above ${formatBytes(
                  SERVER_MAX_INPUT_BYTES,
                )}, regardless of this setting.`}
              >
                {MAX_INPUT_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label} · {formatNumber(option.value)} bytes
                  </option>
                ))}
              </Select>
              <div className="flex items-end">
                <div className="w-full border border-line bg-paper/50 px-3 py-2.5">
                  <p className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
                    Server hard limit
                  </p>
                  <p className="mt-1 font-mono text-[12px] text-ink">
                    {formatBytes(SERVER_MAX_INPUT_BYTES)} JSON body / input
                  </p>
                </div>
              </div>
            </div>
          </SettingsSection>

          <SettingsSection
            index="02"
            title="Transformation options"
            description="Disable a transformation when its output is not appropriate for your workflow. Protection rules always stay on."
          >
            <div className="grid gap-x-8 sm:grid-cols-2">
              <div className="divide-y divide-line">
                <Toggle
                  checked={draft.collapseRepetitions}
                  onChange={(checked) => updateDraft({ collapseRepetitions: checked })}
                  label="Collapse repeated lines"
                  description="Consecutive exact repeats outside protected evidence."
                />
                <Toggle
                  checked={draft.collapsePassingTests}
                  onChange={(checked) => updateDraft({ collapsePassingTests: checked })}
                  label="Collapse passing tests"
                  description="Contiguous passing entries; failures and adjacent evidence stay."
                />
              </div>
              <div className="divide-y divide-line">
                <Toggle
                  checked={draft.collapseProgressUpdates}
                  onChange={(checked) => updateDraft({ collapseProgressUpdates: checked })}
                  label="Collapse progress updates"
                  description="Repeated progress states, retaining the final update."
                />
                <Toggle
                  checked={draft.structuralJson}
                  onChange={(checked) => updateDraft({ structuralJson: checked })}
                  label="Structural JSON dedupe"
                  description="Large object arrays; emits a schema-change warning and marker."
                />
              </div>
            </div>
            <p className="mt-3 border-t border-line pt-3 text-[11px] leading-5 text-muted">
              Validation and fallback behavior cannot be disabled. If protected evidence cannot be
              validated, or the candidate is not smaller, the original input is returned unchanged.
            </p>
          </SettingsSection>

          <SettingsSection
            index="03"
            title="Optional cost estimate"
            description="Show an input-token-only cost signal in compression reports. This is a simple arithmetic estimate, not a bill."
          >
            <div className="flex items-start gap-3 border-b border-line pb-4">
              <span className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center border border-line-strong bg-paper text-ink-soft">
                <DollarSign aria-hidden="true" className="h-4 w-4" />
              </span>
              <Toggle
                checked={draft.pricingEnabled}
                onChange={(checked) => updateDraft({ pricingEnabled: checked })}
                label="Enable input-cost estimate"
                description="No provider API key is requested. Only your configured rate is used."
              />
            </div>
            <div className="mt-4 grid gap-4 sm:grid-cols-2">
              <label className="block">
                <span className="mb-1.5 block font-mono text-[10px] font-medium uppercase tracking-[0.12em] text-ink-soft">
                  Input price per 1M tokens
                </span>
                <input
                  type="number"
                  inputMode="decimal"
                  min={0}
                  max={1_000_000}
                  step="any"
                  value={draft.pricePerMillion}
                  disabled={!draft.pricingEnabled}
                  onChange={(event) => updateDraft({ pricePerMillion: event.target.value })}
                  aria-invalid={draft.pricingEnabled && errors.length > 0 ? true : undefined}
                  className="field-input h-10 w-full"
                  placeholder="e.g. 3.00"
                />
                <span className="mt-1.5 block text-[11px] leading-5 text-muted">
                  Use the input-token rate from your own provider.
                </span>
              </label>
              <label className="block">
                <span className="mb-1.5 block font-mono text-[10px] font-medium uppercase tracking-[0.12em] text-ink-soft">
                  Currency
                </span>
                <input
                  type="text"
                  value={draft.currency}
                  maxLength={8}
                  disabled={!draft.pricingEnabled}
                  onChange={(event) =>
                    updateDraft({ currency: event.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "") })
                  }
                  aria-invalid={draft.pricingEnabled && errors.length > 0 ? true : undefined}
                  className="field-input h-10 w-full uppercase"
                  placeholder="USD"
                />
                <span className="mt-1.5 block text-[11px] leading-5 text-muted">
                  A 3–8 character currency or unit code.
                </span>
              </label>
            </div>
            <p className="mt-3 text-[11px] leading-5 text-muted">
              Cost estimates only apply to input tokens and exclude cache, output, and provider
              minimum charges. They are never presented as exact billed costs.
            </p>
          </SettingsSection>

          <div className="flex flex-wrap items-center justify-between gap-3 border border-line bg-surface px-4 py-3">
            <p className="text-[11.5px] leading-5 text-muted">
              {dirty
                ? "You have unsaved changes."
                : "All changes are saved in this browser."}
            </p>
            <div className="flex gap-2">
              <Button variant="secondary" size="sm" onClick={discard} disabled={!dirty}>
                Discard
              </Button>
              <Button variant="primary" size="sm" onClick={save} disabled={!canSave}>
                <Check aria-hidden="true" className="h-3.5 w-3.5" />
                Save changes
              </Button>
            </div>
          </div>
        </div>

        <aside className="space-y-4">
          <section className="border border-line bg-surface p-4 sm:p-5">
            <div className="flex items-center gap-2">
              <HardDrive aria-hidden="true" className="h-4 w-4 text-ink-soft" />
              <h2 className="text-[14px] font-semibold">Local data</h2>
              <Badge tone={persisted ? "success" : "danger"} className="ml-auto">
                {persisted ? "available" : "unavailable"}
              </Badge>
            </div>
            <div className="mt-4 space-y-3 border-t border-line pt-4 text-[11.5px] leading-5 text-muted">
              <p>
                Settings are stored under{" "}
                <code className="break-all font-mono text-[10.5px] text-ink">
                  contextdrop.settings.v1
                </code>{" "}
                in browser localStorage.
              </p>
              <p>
                Input text is not persisted. Result downloads are created on demand in this browser.
              </p>
              {!persisted ? (
                <div className="flex items-start gap-2 border border-amber/30 bg-amber-soft/45 p-2.5 text-amber-dark">
                  <TriangleAlert aria-hidden="true" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  <span>Storage is blocked or unavailable. Settings will reset on reload.</span>
                </div>
              ) : null}
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
              <Button variant="secondary" size="sm" onClick={exportSettings}>
                <Download aria-hidden="true" className="h-3.5 w-3.5" />
                Export settings
              </Button>
              <Link
                to="/rules"
                className="inline-flex min-h-9 items-center border border-transparent px-3 py-1.5 text-[12px] font-medium text-rust hover:bg-paper-deep/70 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
              >
                {pluralize(settings.preservePatterns.length, "custom pattern")} →
              </Link>
            </div>
          </section>

          <section className="border border-line bg-surface p-4 sm:p-5">
            <div className="flex items-center gap-2">
              <Server aria-hidden="true" className="h-4 w-4 text-ink-soft" />
              <h2 className="text-[14px] font-semibold">Local server</h2>
              {health.state === "online" ? (
                <Badge tone="success" className="ml-auto">
                  online
                </Badge>
              ) : health.state === "loading" ? (
                <Badge className="ml-auto">checking</Badge>
              ) : (
                <Badge tone="danger" className="ml-auto">
                  offline
                </Badge>
              )}
            </div>
            {health.state === "online" ? (
              <dl className="mt-4 divide-y divide-line border-y border-line text-[11.5px]">
                <div className="flex items-center justify-between gap-3 py-2">
                  <dt className="text-muted">Engine</dt>
                  <dd className="font-mono">v{health.data.engine_version}</dd>
                </div>
                <div className="flex items-center justify-between gap-3 py-2">
                  <dt className="text-muted">API schema</dt>
                  <dd className="font-mono">{health.data.api_version}</dd>
                </div>
                <div className="flex items-center justify-between gap-3 py-2">
                  <dt className="text-muted">Input ceiling</dt>
                  <dd className="font-mono">{formatBytes(health.data.max_input_bytes)}</dd>
                </div>
              </dl>
            ) : health.state === "offline" ? (
              <div className="mt-3">
                <ErrorNotice
                  title="Local API unavailable"
                  message={health.message}
                  onRetry={() => setHealthAttempt((value) => value + 1)}
                  retryLabel="Retry"
                />
              </div>
            ) : (
              <div className="mt-4 space-y-2">
                <div className="skeleton h-5 w-full" />
                <div className="skeleton h-5 w-4/5" />
              </div>
            )}
            <p className="mt-3 font-mono text-[9px] uppercase leading-5 tracking-[0.07em] text-muted">
              Loopback only · strict Host/Origin checks
              <br />
              No telemetry · no database required
            </p>
          </section>

          <section className="border border-moss/25 bg-moss-soft/35 p-4 sm:p-5">
            <div className="flex items-center gap-2">
              <ShieldCheck aria-hidden="true" className="h-4 w-4 text-moss" />
              <h2 className="text-[14px] font-semibold text-moss">Privacy boundary</h2>
            </div>
            <p className="mt-3 text-[11.5px] leading-5 text-ink-soft">
              Input is sent only to the loopback ContextDrop API for processing. There are no model
              calls, third-party requests, accounts, or analytics. Closing the server ends the
              boundary.
            </p>
            <p className="mt-2 text-[11px] leading-5 text-muted">
              Do not expose the server beyond loopback. See the repository privacy and threat-model
              documentation for the full boundary.
            </p>
          </section>

          <section className="border border-line bg-surface p-4 sm:p-5">
            <div className="flex items-start gap-2">
              <SlidersHorizontal aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-ink-soft" />
              <div>
                <h2 className="text-[14px] font-semibold">Restore defaults</h2>
                <p className="mt-1 text-[11.5px] leading-5 text-muted">
                  Resets tokenizer, strategy, limits, pricing, and custom preserve patterns. This
                  cannot be undone without saved settings.
                </p>
              </div>
            </div>
            {!confirmReset ? (
              <Button
                variant="danger"
                size="sm"
                className="mt-3"
                onClick={() => setConfirmReset(true)}
              >
                <RotateCcw aria-hidden="true" className="h-3.5 w-3.5" />
                Restore defaults
              </Button>
            ) : (
              <div className="mt-3 border border-danger/25 bg-danger-soft/35 p-3">
                <p className="text-[11.5px] leading-5 text-danger">
                  Also clears {pluralize(settings.preservePatterns.length, "custom pattern")}? This
                  removes them from this browser.
                </p>
                <div className="mt-2 flex gap-2">
                  <Button variant="danger" size="sm" onClick={restoreDefaults}>
                    Yes, restore defaults
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setConfirmReset(false)}
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            )}
          </section>
        </aside>
      </div>
    </>
  );
}
