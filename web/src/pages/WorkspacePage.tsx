import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  Braces,
  ChevronDown,
  Download,
  FileCode2,
  RotateCcw,
  ScanLine,
  ShieldCheck,
  SlidersHorizontal,
  Upload,
  X,
} from "lucide-react";
import {
  compressInput,
  errorMessage,
  getFixture,
  getFixtures,
  isAbortError,
} from "../api";
import type {
  CompressionRequest,
  CompressionResult,
  FixtureSummary,
  InputFormat,
  Strategy,
  TokenizerKind,
} from "../types";
import {
  MAX_INPUT_OPTIONS,
  SERVER_MAX_INPUT_BYTES,
  useSettings,
  type Settings,
} from "../settings";
import {
  cn,
  countLines,
  downloadText,
  fileExtension,
  formatBytes,
  formatCost,
  formatMs,
  formatNumber,
  formatPercent,
  inputFormatLabel,
  pluralize,
  pricingFromSettings,
  safeDownloadName,
  strategyDescription,
  strategyLabel,
  tokenizerShortLabel,
  utf8Bytes,
  warningsForDisplay,
} from "../utils";
import { CopyButton } from "../components/CopyButton";
import { CodeBlock } from "../components/CodeBlock";
import { DiffView } from "../components/DiffView";
import { StrategyPicker } from "../components/StrategyPicker";
import {
  Badge,
  Button,
  ErrorNotice,
  Kbd,
  PageHeader,
  SectionHeading,
  Select,
  Toggle,
} from "../components/ui";

interface RunConfig {
  strategy: Strategy;
  tokenizer: TokenizerKind;
  inputFormat: InputFormat;
  maxInputBytes: number;
  collapseRepetitions: boolean;
  collapsePassingTests: boolean;
  collapseProgressUpdates: boolean;
  structuralJson: boolean;
  preservePatterns: string[];
  pricingEnabled: boolean;
  pricePerMillion: string;
  currency: string;
}

interface LastRun {
  result: CompressionResult;
  input: string;
  signature: string;
  sourceLabel: string | null;
  ranAt: number;
}

interface UiError {
  title: string;
  message: string;
}

type ResultView = "output" | "diff" | "evidence" | "json";

function configFromSettings(settings: Settings): RunConfig {
  return {
    strategy: settings.strategy,
    tokenizer: settings.tokenizer,
    inputFormat: settings.inputFormat,
    maxInputBytes: settings.maxInputBytes,
    collapseRepetitions: settings.collapseRepetitions,
    collapsePassingTests: settings.collapsePassingTests,
    collapseProgressUpdates: settings.collapseProgressUpdates,
    structuralJson: settings.structuralJson,
    preservePatterns: [...settings.preservePatterns],
    pricingEnabled: settings.pricingEnabled,
    pricePerMillion: settings.pricePerMillion,
    currency: settings.currency,
  };
}

function signatureOf(config: RunConfig): string {
  return JSON.stringify(config);
}

function buildPayload(text: string, config: RunConfig): CompressionRequest {
  const pricing = pricingFromSettings(
    config.pricingEnabled,
    config.pricePerMillion,
    config.currency,
  );
  return {
    text,
    strategy: config.strategy,
    input_format: config.inputFormat,
    tokenizer: config.tokenizer,
    preserve_patterns: config.preservePatterns,
    ...(pricing ? { pricing } : {}),
    collapse_repetitions: config.collapseRepetitions,
    collapse_passing_tests: config.collapsePassingTests,
    collapse_progress_updates: config.collapseProgressUpdates,
    structural_json: config.structuralJson,
    max_input_bytes: config.maxInputBytes,
  };
}

function omissionParts(result: CompressionResult): Array<{ label: string; count: number }> {
  const o = result.omissions;
  return [
    { label: "Repeated lines", count: o.repeated_lines_omitted },
    { label: "Passing-test lines", count: o.passing_test_lines_omitted },
    { label: "Progress updates", count: o.progress_lines_omitted },
    { label: "Normalized log lines", count: o.normalized_log_lines_omitted },
    { label: "Duplicate JSON records", count: o.json_duplicate_items_omitted },
  ].filter((entry) => entry.count > 0);
}

function MetricsStrip({ lastRun }: { lastRun: LastRun | null }) {
  const metrics = lastRun?.result.metrics;
  const preservation = lastRun?.result.preservation;

  const cards = [
    {
      label: "Byte reduction",
      value: metrics ? `${metrics.byte_reduction_percent.toFixed(1)}%` : "—",
      detail: metrics
        ? `${formatNumber(metrics.original_bytes)} → ${formatNumber(metrics.output_bytes)} bytes`
        : "No run yet",
      accent: metrics !== undefined,
    },
    {
      label: "Token reduction",
      value: metrics ? `${metrics.token_reduction_percent.toFixed(1)}%` : "—",
      detail: metrics
        ? `${formatNumber(metrics.original_tokens)} → ${formatNumber(metrics.output_tokens)} · ${metrics.token_measurement.name}`
        : "Exact tokens by default",
      accent: metrics !== undefined,
    },
    {
      label: "Evidence retained",
      value: preservation ? `${preservation.retained_occurrences}/${preservation.protected_occurrences}` : "—",
      detail: preservation
        ? preservation.protected_occurrences === 0
          ? "No protected occurrences detected"
          : preservation.passed
            ? "Preservation validation passed"
            : "Preservation validation failed"
        : "Rule checks run locally",
      accent: preservation?.passed === true,
      danger: preservation?.passed === false,
    },
    {
      label: "Process time",
      value: metrics ? formatMs(metrics.processing_time_ms) : "—",
      detail: metrics
        ? `${strategyLabel(lastRun!.result.strategy)} · ${lastRun!.result.classification.kind.replaceAll("-", " ")}`
        : "Transformation time",
      accent: metrics !== undefined,
    },
  ];

  return (
    <section aria-label="Result metrics" className="mt-4 border-y border-line bg-surface">
      <div className="grid grid-cols-2 divide-x divide-line xl:grid-cols-4">
        {cards.map((card, index) => (
          <div
            key={card.label}
            className={cn(
              "min-w-0 px-4 py-5 sm:px-5",
              index >= 2 && "border-t border-line xl:border-t-0",
            )}
          >
            <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-muted">
              {card.label}
            </p>
            <p
              key={`${card.value}-${lastRun?.ranAt ?? 0}`}
              className={cn(
                "metric-pop mt-2 font-display text-[2.35rem] leading-none tracking-[-0.02em]",
                card.danger ? "text-danger" : card.accent ? "text-ink" : "text-muted/45",
              )}
            >
              {card.value}
            </p>
            <p className="mt-2 line-clamp-2 min-h-8 font-mono text-[9.5px] leading-4 text-muted">
              {card.detail}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function InitialOutput({ onLoadSample }: { onLoadSample: () => void }) {
  return (
    <div className="empty-grid flex min-h-[440px] flex-col p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-muted">Output stage</p>
          <p className="mt-1 text-[13px] text-ink-soft">No transformation has run.</p>
        </div>
        <span className="grid h-9 w-9 place-items-center border border-line-strong bg-surface text-muted">
          <Braces aria-hidden="true" className="h-4 w-4" strokeWidth={1.5} />
        </span>
      </div>

      <div className="my-auto py-8 text-center">
        <span className="mx-auto mb-5 grid h-12 w-12 place-items-center border border-ink/20 bg-surface text-ink">
          <ScanLine aria-hidden="true" className="h-5 w-5" strokeWidth={1.5} />
        </span>
        <h3 className="mx-auto max-w-xs font-display text-[2.1rem] leading-[0.95] tracking-[-0.02em]">
          The distilled context appears here.
        </h3>
        <p className="mx-auto mt-3 max-w-sm text-[12.5px] leading-5 text-muted">
          Every result includes the processed output, real before/after metrics, omission markers,
          and preservation checks.
        </p>
        <Button variant="secondary" size="sm" className="mt-5" onClick={onLoadSample}>
          Load a failure sample
          <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-px border border-line bg-line">
        {["Process", "Validate", "Measure"].map((label, index) => (
          <div key={label} className="bg-surface/90 px-3 py-3">
            <span className="font-mono text-[9px] text-rust">0{index + 1}</span>
            <p className="mt-1 text-[11px] font-medium text-ink-soft">{label}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function ResultEvidence({ lastRun }: { lastRun: LastRun }) {
  const result = lastRun.result;
  const warnings = warningsForDisplay(result.warnings);
  const omissions = omissionParts(result);
  const metrics = result.metrics;

  return (
    <div className="space-y-5 p-4 sm:p-5">
      <div
        className={cn(
          "flex flex-wrap items-center justify-between gap-3 border px-3.5 py-3",
          result.preservation.passed
            ? "border-moss/25 bg-moss-soft/60"
            : "border-danger/30 bg-danger-soft/60",
        )}
      >
        <div className="flex items-start gap-3">
          <ShieldCheck
            aria-hidden="true"
            className={cn("mt-0.5 h-4 w-4", result.preservation.passed ? "text-moss" : "text-danger")}
          />
          <div>
            <p className={cn("text-sm font-semibold", result.preservation.passed ? "text-moss" : "text-danger")}>
              {result.preservation.passed ? "Preservation validation passed" : "Preservation validation failed"}
            </p>
            <p className="mt-0.5 text-[12px] leading-5 text-ink-soft">
              {formatNumber(result.preservation.retained_occurrences)} of{" "}
              {formatNumber(result.preservation.protected_occurrences)} protected occurrences retained.
            </p>
          </div>
        </div>
        <Badge tone={result.preservation.passed ? "success" : "danger"}>
          {result.fallback_used ? "Fallback returned" : "Output accepted"}
        </Badge>
      </div>

      {result.fallback_used ? (
        <div className="border border-amber/30 bg-amber-soft/55 px-3.5 py-3 text-[12.5px] leading-5 text-amber-dark">
          <span className="font-semibold">Original input returned.</span>{" "}
          {result.fallback_reason ?? "No byte savings were produced, so the safe fallback was used."}
        </div>
      ) : null}

      <div className="grid gap-5 lg:grid-cols-2">
        <div>
          <SectionHeading title="Run classification" />
          <dl className="divide-y divide-line border-y border-line text-[12px]">
            {[
              ["Detected format", inputFormatLabel(result.classification.kind)],
              [
                "Classification",
                `${result.classification.automatic ? "Automatic" : "Manual"} · ${formatPercent(
                  result.classification.confidence * 100,
                  0,
                )} confidence`,
              ],
              ["Strategy", strategyLabel(result.strategy)],
              [
                "Tokens",
                `${result.metrics.token_measurement.name} · ${
                  result.metrics.token_measurement.kind === "exact" ? "exact" : "estimated"
                }`,
              ],
              ["Processing time", formatMs(result.metrics.processing_time_ms)],
            ].map(([label, value]) => (
              <div key={label} className="flex items-start justify-between gap-4 py-2">
                <dt className="text-muted">{label}</dt>
                <dd className="text-right font-medium text-ink">{value}</dd>
              </div>
            ))}
          </dl>
        </div>

        <div>
          <SectionHeading title="Omissions" description="Only non-protected spans are eligible." />
          {omissions.length === 0 ? (
            <p className="border-y border-line py-4 text-[12.5px] text-muted">
              No content was omitted. The output either changed formatting only or returned the
              original input.
            </p>
          ) : (
            <dl className="divide-y divide-line border-y border-line text-[12px]">
              {omissions.map((entry) => (
                <div key={entry.label} className="flex items-center justify-between gap-4 py-2">
                  <dt className="text-muted">{entry.label}</dt>
                  <dd className="font-mono text-ink">{formatNumber(entry.count)}</dd>
                </div>
              ))}
              <div className="flex items-center justify-between gap-4 py-2">
                <dt className="text-muted">Explicit markers inserted</dt>
                <dd className="font-mono text-ink">{formatNumber(result.omissions.markers_inserted)}</dd>
              </div>
            </dl>
          )}
        </div>
      </div>

      {result.preservation.checks.length > 0 ? (
        <div>
          <SectionHeading title="Preservation checks" />
          <ul className="divide-y divide-line border-y border-line">
            {result.preservation.checks.map((check) => (
              <li key={check.id} className="flex items-start gap-3 py-2.5 text-[12.5px]">
                <span
                  aria-hidden="true"
                  className={cn(
                    "mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full",
                    check.passed ? "bg-moss" : "bg-danger",
                  )}
                />
                <div className="min-w-0 flex-1">
                  <p className="font-medium text-ink">{check.label}</p>
                  <p className="mt-0.5 text-[11.5px] leading-4 text-muted">{check.detail}</p>
                </div>
                <Badge tone={check.passed ? "success" : "danger"}>{check.passed ? "pass" : "fail"}</Badge>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {warnings.length > 0 ? (
        <div>
          <SectionHeading title="Warnings" description="Transparent limits from this run." />
          <ul className="space-y-2">
            {warnings.map((warning) => (
              <li
                key={warning}
                className="border-l-2 border-amber bg-amber-soft/35 px-3 py-2 text-[12px] leading-5 text-amber-dark"
              >
                {warning}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {metrics.cost ? (
        <div>
          <SectionHeading
            title="Optional cost signal"
            description="Input tokens only; not a billing guarantee."
          />
          <div className="grid grid-cols-3 divide-x divide-line border-y border-line">
            <div className="px-3 py-3">
              <p className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">Original</p>
              <p className="mt-1 font-mono text-[12px]">
                {formatCost(metrics.cost, metrics.cost.original_input_cost)}
              </p>
            </div>
            <div className="px-3 py-3">
              <p className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">Compressed</p>
              <p className="mt-1 font-mono text-[12px]">
                {formatCost(metrics.cost, metrics.cost.compressed_input_cost)}
              </p>
            </div>
            <div className="px-3 py-3">
              <p className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">Input saved</p>
              <p className="mt-1 font-mono text-[12px] text-moss">
                {formatCost(metrics.cost, metrics.cost.input_cost_saved)}
              </p>
            </div>
          </div>
          <p className="mt-2 text-[11px] leading-4 text-muted">{metrics.cost.note}</p>
        </div>
      ) : null}

      <p className="border-t border-line pt-3 text-[11px] leading-5 text-muted">
        {result.preservation.note} This report does not claim semantic equivalence or guaranteed
        savings.
      </p>
    </div>
  );
}

export function WorkspacePage() {
  const { settings } = useSettings();
  const [config, setConfig] = useState<RunConfig>(() => configFromSettings(settings));
  const [text, setText] = useState("");
  const [sourceLabel, setSourceLabel] = useState<string | null>(null);
  const [lastRun, setLastRun] = useState<LastRun | null>(null);
  const [view, setView] = useState<ResultView>("output");
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const [fixtures, setFixtures] = useState<FixtureSummary[]>([]);
  const [fixtureError, setFixtureError] = useState<string | null>(null);
  const [fixtureAttempt, setFixtureAttempt] = useState(0);
  const [selectedSample, setSelectedSample] = useState("");
  const [sampleBusy, setSampleBusy] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const compressAbort = useRef<AbortController | null>(null);
  const sampleAbort = useRef<AbortController | null>(null);

  useEffect(() => {
    setConfig(configFromSettings(settings));
  }, [settings]);

  useEffect(() => {
    const controller = new AbortController();
    setFixtureError(null);
    getFixtures(controller.signal)
      .then(setFixtures)
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setFixtureError(errorMessage(caught));
        }
      });
    return () => controller.abort();
  }, [fixtureAttempt]);

  useEffect(() => {
    return () => {
      compressAbort.current?.abort();
      sampleAbort.current?.abort();
    };
  }, []);

  const inputBytes = useMemo(() => utf8Bytes(text), [text]);
  const lineCount = useMemo(() => countLines(text), [text]);
  const overLimit = inputBytes > config.maxInputBytes;
  const configSignature = useMemo(() => signatureOf(config), [config]);
  const stale =
    lastRun !== null &&
    (lastRun.input !== text || lastRun.signature !== configSignature);
  const canRun = text.length > 0 && !busy && !overLimit;

  const resetSource = useCallback(() => {
    setSourceLabel(null);
    setSelectedSample("");
  }, []);

  const runCompression = useCallback(async () => {
    if (!canRun) {
      return;
    }
    compressAbort.current?.abort();
    const controller = new AbortController();
    compressAbort.current = controller;
    setBusy(true);
    setError(null);
    const input = text;
    try {
      const result = await compressInput(buildPayload(input, config), controller.signal);
      setLastRun({
        result,
        input,
        signature: configSignature,
        sourceLabel,
        ranAt: Date.now(),
      });
      setView("output");
    } catch (caught: unknown) {
      if (!isAbortError(caught)) {
        setError({ title: "Compression failed", message: errorMessage(caught) });
      }
    } finally {
      if (compressAbort.current === controller) {
        compressAbort.current = null;
        setBusy(false);
      }
    }
  }, [canRun, config, configSignature, sourceLabel, text]);

  const loadSample = useCallback(async (fixtureId: string) => {
    if (!fixtureId) {
      return;
    }
    sampleAbort.current?.abort();
    compressAbort.current?.abort();
    const controller = new AbortController();
    sampleAbort.current = controller;
    setSelectedSample(fixtureId);
    setSampleBusy(true);
    setError(null);
    try {
      const detail = await getFixture(fixtureId, controller.signal);
      setText(detail.content);
      setSourceLabel(detail.fixture.name);
      setLastRun(null);
      setView("output");
      setSelectedSample("");
    } catch (caught: unknown) {
      if (!isAbortError(caught)) {
        setError({ title: "Could not load sample", message: errorMessage(caught) });
        setSelectedSample("");
      }
    } finally {
      if (sampleAbort.current === controller) {
        sampleAbort.current = null;
        setSampleBusy(false);
      }
    }
  }, []);

  const readFile = useCallback(
    async (files: FileList | null) => {
      const file = files?.[0];
      if (!file) {
        return;
      }
      compressAbort.current?.abort();
      setError(null);
      if (file.size > config.maxInputBytes) {        setError({
          title: "File is too large",
          message: `This file is ${formatBytes(file.size)}; the current limit is ${formatBytes(
            config.maxInputBytes,
          )}. Raise the limit in Settings or choose a smaller file.`,
        });
        return;
      }
      try {
        const bytes = await file.arrayBuffer();
        const decoded = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
        if (utf8Bytes(decoded) > config.maxInputBytes) {
          setError({
            title: "File is too large",
            message: `The decoded input exceeds the ${formatBytes(config.maxInputBytes)} limit.`,
          });
          return;
        }
        setText(decoded);
        setSourceLabel(file.name);
        setLastRun(null);
        setView("output");
        setSelectedSample("");
      } catch {
        setError({
          title: "Could not read file",
          message: "The selected file is not valid UTF-8 text or could not be read by this browser.",
        });
      }
    },
    [config.maxInputBytes],
  );

  const clearInput = useCallback(() => {
    compressAbort.current?.abort();
    setText("");
    setSourceLabel(null);
    setLastRun(null);
    setError(null);
    setView("output");
    setSelectedSample("");
  }, []);

  function updateConfig(patch: Partial<RunConfig>) {
    setConfig((current) => ({ ...current, ...patch }));
    setError(null);
  }

  function resetConfig() {
    setConfig(configFromSettings(settings));
    setError(null);
  }

  const displayValue =
    lastRun === null
      ? ""
      : view === "json"
        ? JSON.stringify(lastRun.result, null, 2)
        : lastRun.result.output;

  const copyMode = view === "json" ? "JSON" : "output";
  const downloadLabel = view === "json" ? "Download result JSON" : "Download processed output";

  function downloadCurrent() {
    if (!lastRun) {
      return;
    }
    const baseName = `contextdrop-${safeDownloadName(
      lastRun.sourceLabel ?? lastRun.result.strategy,
    )}`;
    if (view === "json") {
      downloadText(`${baseName}-result.json`, displayValue, "application/json;charset=utf-8");
    } else {
      downloadText(
        `${baseName}.${fileExtension(lastRun.result.classification.kind)}`,
        displayValue,
      );
    }
  }

  const liveMessage = busy
    ? "Compressing locally."
    : lastRun && !stale
      ? `Run complete. ${formatPercent(lastRun.result.metrics.byte_reduction_percent)} byte reduction; preservation ${
          lastRun.result.preservation.passed ? "passed" : "failed"
        }.`
      : "";

  const warnings = lastRun ? warningsForDisplay(lastRun.result.warnings) : [];
  const outputLines = lastRun ? countLines(lastRun.result.output) : 0;

  return (
    <>
      <PageHeader
        index="01 / Workspace"
        title="Workspace"
        description="Drop in agent context, logs, or test output. ContextDrop removes deterministic noise while protecting the diagnostic evidence you need."
        actions={
          <Link
            to="/benchmark"
            className="inline-flex min-h-10 items-center gap-2 border border-line-strong bg-surface px-3.5 py-2 text-[13px] font-medium text-ink transition-colors hover:border-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
          >
            Benchmark Lab
            <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
          </Link>
        }
        meta={
          <>
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Default <span className="text-ink">{strategyLabel(settings.strategy)}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Tokenizer <span className="text-ink">{tokenizerShortLabel(settings.tokenizer)}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Limit <span className="text-ink">{formatBytes(settings.maxInputBytes)}</span>
            </span>
            {settings.preservePatterns.length > 0 ? (
              <>
                <span className="hidden h-3 w-px bg-line sm:block" />
                <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
                  Custom rules{" "}
                  <span className="text-ink">{formatNumber(settings.preservePatterns.length)}</span>
                </span>
              </>
            ) : null}
            <Link
              to="/settings"
              className="ml-auto font-mono text-[9px] uppercase tracking-[0.1em] text-rust hover:text-rust-dark hover:underline"
            >
              Edit defaults
            </Link>
          </>
        }
      />

      <div className="sr-only" role="status" aria-live="polite">
        {liveMessage}
      </div>

      <section aria-label="Compression controls" className="border border-line bg-surface">
        <div className="border-b border-line p-3 sm:p-4">
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
            <div>
              <p className="mb-2 font-mono text-[9px] uppercase tracking-[0.14em] text-muted">
                Compression strategy
              </p>
              <StrategyPicker
                value={config.strategy}
                onChange={(strategy) => updateConfig({ strategy })}
                disabled={busy}
              />
              <p className="mt-2 max-w-2xl text-[12px] leading-5 text-muted">
                {strategyDescription(config.strategy)}
              </p>
            </div>
            <Button
              variant="primary"
              size="md"
              loading={busy}
              disabled={!canRun}
              onClick={() => void runCompression()}
              className="w-full justify-center px-5 lg:w-auto lg:min-w-[190px]"
            >
              {busy ? "Compressing locally…" : lastRun ? "Run again" : "Compress context"}
              {!busy ? <ArrowRight aria-hidden="true" className="h-4 w-4" /> : null}
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 border-b border-line p-3 sm:px-4">
          <Button
            variant={advancedOpen ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setAdvancedOpen((open) => !open)}
            aria-expanded={advancedOpen}
            aria-controls="advanced-options"
          >
            <SlidersHorizontal aria-hidden="true" className="h-3.5 w-3.5" />
            Options
            <ChevronDown
              aria-hidden="true"
              className={cn("h-3.5 w-3.5 transition-transform", advancedOpen && "rotate-180")}
            />
          </Button>
          <span className="hidden font-mono text-[9px] uppercase tracking-[0.08em] text-muted sm:inline">
            {tokenizerShortLabel(config.tokenizer)} · {inputFormatLabel(config.inputFormat)} ·{" "}
            {formatBytes(config.maxInputBytes)}
          </span>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <Select
              label="Load a built-in sample"
              hideLabel
              value={selectedSample}
              onChange={(event) => void loadSample(event.target.value)}
              disabled={busy || sampleBusy || fixtures.length === 0}
              className="h-9 min-w-[170px] text-[12px]"
              containerClassName="w-auto"
            >
              <option value="">
                {sampleBusy ? "Loading sample…" : fixtures.length === 0 ? "Samples unavailable" : "Load a sample…"}
              </option>
              {fixtures.map((fixture) => (
                <option key={fixture.id} value={fixture.id}>
                  {fixture.name}
                </option>
              ))}
            </Select>
            <input
              ref={fileInputRef}
              type="file"
              accept=".log,.txt,.json,.jsonl,.ndjson,.out,.trace,.md"
              onChange={(event) => {
                const files = event.currentTarget.files;
                void readFile(files);
                event.currentTarget.value = "";
              }}
            />
            <Button variant="ghost" size="sm" onClick={() => fileInputRef.current?.click()}>
              <Upload aria-hidden="true" className="h-3.5 w-3.5" />
              Open file
            </Button>
          </div>
        </div>

        {advancedOpen ? (
          <div id="advanced-options" className="border-b border-line bg-paper/45 p-3 sm:p-4">
            <div className="grid gap-4 md:grid-cols-3">
              <Select
                label="Token measurement"
                value={config.tokenizer}
                onChange={(event) => updateConfig({ tokenizer: event.target.value as TokenizerKind })}
                hint="Exact tokenizers run locally. The estimate has no network dependency either."
              >
                <option value="o200k_base">o200k_base · exact</option>
                <option value="cl100k_base">cl100k_base · exact</option>
                <option value="estimate">Heuristic estimate</option>
              </Select>
              <Select
                label="Input format"
                value={config.inputFormat}
                onChange={(event) => updateConfig({ inputFormat: event.target.value as InputFormat })}
                hint="Auto uses conservative format classification; force a format when needed."
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
                label="Maximum input"
                value={config.maxInputBytes}
                onChange={(event) => updateConfig({ maxInputBytes: Number(event.target.value) })}
                hint={`Server hard limit: ${formatBytes(SERVER_MAX_INPUT_BYTES)} per input.`}
              >
                {MAX_INPUT_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </Select>
            </div>
            <div className="mt-2 grid gap-x-8 border-t border-line pt-1 md:grid-cols-2">
              <div className="divide-y divide-line/70">
                <Toggle
                  checked={config.collapseRepetitions}
                  onChange={(checked) => updateConfig({ collapseRepetitions: checked })}
                  label="Collapse repeated lines"
                  description="Consecutive exact repetitions outside protected evidence."
                />
                <Toggle
                  checked={config.collapsePassingTests}
                  onChange={(checked) => updateConfig({ collapsePassingTests: checked })}
                  label="Collapse passing tests"
                  description="Compress contiguous passing entries; failures remain protected."
                />
              </div>
              <div className="divide-y divide-line/70">
                <Toggle
                  checked={config.collapseProgressUpdates}
                  onChange={(checked) => updateConfig({ collapseProgressUpdates: checked })}
                  label="Collapse progress updates"
                  description="Keep the final state of progress groups."
                />
                <Toggle
                  checked={config.structuralJson}
                  onChange={(checked) => updateConfig({ structuralJson: checked })}
                  label="Structural JSON dedupe"
                  description="Deduplicate large object arrays with an explicit schema-change warning."
                />
              </div>
            </div>
            <div className="mt-2 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-3">
              <p className="text-[11px] leading-5 text-muted">
                {config.preservePatterns.length > 0
                  ? `${pluralize(config.preservePatterns.length, "custom preserve pattern")} active from Rules.`
                  : "No custom preserve patterns are active."}{" "}
                <Link to="/rules" className="text-rust underline decoration-rust/30 hover:decoration-rust">
                  Manage rules
                </Link>
              </p>
              <Button
                variant="ghost"
                size="sm"
                onClick={resetConfig}
                disabled={configSignature === signatureOf(configFromSettings(settings))}
              >
                <RotateCcw aria-hidden="true" className="h-3.5 w-3.5" />
                Use saved defaults
              </Button>
            </div>
          </div>
        ) : null}

        {fixtureError ? (
          <div className="border-b border-line px-3 py-2 text-[11.5px] text-danger sm:px-4">
            Samples could not be loaded: {fixtureError}{" "}
            <button
              type="button"
              onClick={() => setFixtureAttempt((attempt) => attempt + 1)}
              className="font-medium underline underline-offset-2 hover:text-danger/80"
            >
              Retry
            </button>
          </div>
        ) : null}
      </section>

      {error ? (
        <div className="mt-3">
          <ErrorNotice
            title={error.title}
            message={error.message}
            onRetry={error.title === "Compression failed" && canRun ? () => void runCompression() : undefined}
          />
        </div>
      ) : null}

      <div className="mt-4 grid items-start gap-4 xl:grid-cols-2">
        <section
          aria-labelledby="input-heading"
          className={cn(
            "relative border border-line bg-surface",
            dragActive && "border-rust ring-1 ring-rust",
          )}
          onDragEnter={(event) => {
            event.preventDefault();
            setDragActive(true);
          }}
          onDragOver={(event) => {
            event.preventDefault();
            setDragActive(true);
          }}
          onDragLeave={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              setDragActive(false);
            }
          }}
          onDrop={(event) => {
            event.preventDefault();
            setDragActive(false);
            void readFile(event.dataTransfer.files);
          }}
        >
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-3 py-2.5 sm:px-4">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <h2 id="input-heading" className="font-mono text-[10px] font-medium uppercase tracking-[0.14em] text-ink">
                Input
              </h2>
              {sourceLabel ? (
                <Badge tone="neutral" className="max-w-[220px] truncate normal-case tracking-normal">
                  {sourceLabel}
                </Badge>
              ) : null}
              <span className={cn("font-mono text-[10px] text-muted", overLimit && "text-danger")}>
                {formatBytes(inputBytes)} · {pluralize(lineCount, "line")}
              </span>
            </div>
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="sm" onClick={clearInput} disabled={!text || busy}>
                <X aria-hidden="true" className="h-3.5 w-3.5" />
                Clear
              </Button>
            </div>
          </div>

          <textarea
            value={text}
            onChange={(event) => {
              setText(event.target.value);
              resetSource();
              setError(null);
            }}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                void runCompression();
              }
            }}
            aria-label="Input context to compress"
            spellCheck={false}
            placeholder={
              "Paste a build log, test failure, stack trace, JSON array, or terminal transcript…\n\nEverything stays on this machine. Input is sent only to the local ContextDrop API."
            }
            className="field-input field-textarea min-h-[440px] w-full rounded-none border-0 bg-surface px-4 py-4 text-ink"
            style={{ minHeight: "440px" }}
          />
          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-line px-3 py-2 sm:px-4">
            <span className="font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
              UTF-8 text · never uploaded
            </span>
            <span className="flex items-center gap-1.5 font-mono text-[9px] text-muted">
              <Kbd>Ctrl</Kbd>
              <span>+</span>
              <Kbd>Enter</Kbd>
              <span>to compress</span>
            </span>
          </div>

          {overLimit ? (
            <div className="border-t border-danger/30 bg-danger-soft/60 px-4 py-2 text-[12px] text-danger" role="alert">
              Input exceeds the {formatBytes(config.maxInputBytes)} limit. Reduce the input or raise
              the limit in Options.
            </div>
          ) : null}

          {dragActive ? (
            <div className="pointer-events-none absolute inset-2 grid place-items-center border-2 border-dashed border-rust bg-paper/90">
              <div className="text-center">
                <Upload aria-hidden="true" className="mx-auto h-5 w-5 text-rust" />
                <p className="mt-2 text-sm font-medium">Drop a UTF-8 text file</p>
              </div>
            </div>
          ) : null}
        </section>

        <section aria-labelledby="output-heading" className="border border-line bg-surface">
          <div className="flex min-h-[53px] flex-wrap items-center justify-between gap-2 border-b border-line px-3 py-2 sm:px-4">
            <div className="flex flex-wrap items-center gap-2">
              <h2 id="output-heading" className="font-mono text-[10px] font-medium uppercase tracking-[0.14em] text-ink">
                Output
              </h2>
              {lastRun ? (
                <>
                  <Badge tone={lastRun.result.fallback_used ? "warning" : "success"}>
                    {lastRun.result.fallback_used ? "Fallback" : "Complete"}
                  </Badge>
                  <span className="font-mono text-[10px] text-muted">
                    {formatBytes(lastRun.result.metrics.output_bytes)} · {pluralize(outputLines, "line")}
                  </span>
                </>
              ) : (
                <span className="font-mono text-[10px] text-muted">No run yet</span>
              )}
            </div>
            {lastRun ? (
              <div className="flex items-center gap-1">
                {warnings.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setView("evidence")}
                    className="mr-1 font-mono text-[9px] uppercase tracking-[0.08em] text-amber-dark underline decoration-amber/40 underline-offset-2 hover:decoration-amber"
                  >
                    {pluralize(warnings.length, "warning")}
                  </button>
                ) : null}
                <CopyButton value={displayValue} label={`Copy ${copyMode}`} />
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={downloadCurrent}
                  aria-label={downloadLabel}
                  title={downloadLabel}
                >
                  <Download aria-hidden="true" className="h-4 w-4" />
                </Button>
              </div>
            ) : null}
          </div>

          {lastRun ? (
            <div className="flex flex-wrap items-center gap-1 border-b border-line px-3 py-1.5 sm:px-4">
              {(
                [
                  ["output", "Output"],
                  ["diff", "Diff"],
                  ["evidence", "Evidence"],
                  ["json", "JSON"],
                ] as Array<[ResultView, string]>
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  aria-pressed={view === value}
                  onClick={() => setView(value)}
                  className={cn(
                    "min-h-9 border-b-2 px-2.5 text-[12px] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust",
                    view === value
                      ? "border-rust text-ink"
                      : "border-transparent text-muted hover:text-ink",
                  )}
                >
                  {label}
                </button>
              ))}
              <span className="ml-auto hidden font-mono text-[9px] uppercase tracking-[0.08em] text-muted sm:inline">
                {strategyLabel(lastRun.result.strategy)} ·{" "}
                {lastRun.ranAt ? new Date(lastRun.ranAt).toLocaleTimeString() : "just now"}
              </span>
            </div>
          ) : null}

          {stale ? (
            <div className="border-b border-amber/30 bg-amber-soft/45 px-4 py-2 text-[11.5px] leading-5 text-amber-dark">
              Input or options changed after this run. The result below is from the earlier input.
              Run again to refresh it.
            </div>
          ) : null}

          {lastRun ? (
            view === "output" ? (
              <div className="p-3">
                <CodeBlock
                  value={lastRun.result.output}
                  ariaLabel="Processed output"
                  maxHeight="32rem"
                  header={
                    <>
                      <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-code-muted">
                        contextdrop://processed
                      </span>
                      <span className="font-mono text-[9px] text-code-muted">
                        {formatBytes(lastRun.result.metrics.output_bytes)}
                      </span>
                    </>
                  }
                  footer={
                    lastRun.result.fallback_used
                      ? "Original returned by the safety fallback."
                      : "Omission markers are explicit and deterministic."
                  }
                />
              </div>
            ) : view === "diff" ? (
              <div className="p-3">
                <DiffView original={lastRun.input} output={lastRun.result.output} />
              </div>
            ) : view === "evidence" ? (
              <ResultEvidence lastRun={lastRun} />
            ) : (
              <div className="p-3">
                <CodeBlock
                  value={JSON.stringify(lastRun.result, null, 2)}
                  ariaLabel="Complete compression result JSON"
                  maxHeight="32rem"
                  header={
                    <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-code-muted">
                      machine-readable result
                    </span>
                  }
                />
              </div>
            )
          ) : (
            <InitialOutput
              onLoadSample={() => {
                const preferred =
                  fixtures.find((fixture) => fixture.id === "rust-build-failure") ??
                  fixtures.find(
                    (fixture) =>
                      fixture.category === "Build failure" || fixture.category === "Test failure",
                  ) ??
                  fixtures[0];
                if (preferred) {
                  void loadSample(preferred.id);
                } else {
                  fileInputRef.current?.click();
                }
              }}
            />
          )}
        </section>
      </div>

      <MetricsStrip lastRun={lastRun} />

      <div className="mt-5 grid gap-4 border border-line bg-surface p-4 sm:p-5 lg:grid-cols-[1.15fr_0.85fr]">
        <div className="flex items-start gap-3">
          <span className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center border border-moss/25 bg-moss-soft text-moss">
            <ShieldCheck aria-hidden="true" className="h-4 w-4" />
          </span>
          <div>
            <p className="text-[13px] font-semibold text-ink">Protection wins over compression.</p>
            <p className="mt-1 max-w-xl text-[12px] leading-5 text-muted">
              Errors, warnings, failed assertions, security findings, source locations, exit codes,
              command context, and adjacent evidence are protected. If validation fails or the
              candidate is not smaller, the original text is returned unchanged.
            </p>
          </div>
        </div>
        <div className="flex items-start gap-3 border-t border-line pt-4 lg:border-l lg:border-t-0 lg:pl-5 lg:pt-0">
          <span className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center border border-line-strong bg-paper text-ink-soft">
            <FileCode2 aria-hidden="true" className="h-4 w-4" />
          </span>
          <div>
            <p className="text-[13px] font-semibold text-ink">Evidence, not semantic promises.</p>
            <p className="mt-1 text-[12px] leading-5 text-muted">
              The report measures bytes and tokens and validates protected spans. It does not claim
              semantic equivalence, perfect recall, or guaranteed savings. Review the diff.
            </p>
          </div>
        </div>
      </div>
    </>
  );
}
