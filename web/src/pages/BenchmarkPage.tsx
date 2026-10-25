import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  CheckCircle2,
  Download,
  FileSearch,
  Gauge,
  ShieldCheck,
  TriangleAlert,
  Upload,
} from "lucide-react";
import {
  errorMessage,
  getFixture,
  getFixtures,
  isAbortError,
  runBenchmark,
} from "../api";
import type {
  BenchmarkRequest,
  BenchmarkRun,
  BenchmarkStrategyResult,
  FixtureDetail,
  FixtureSummary,
  InputFormat,
  Strategy,
  TokenizerKind,
} from "../types";
import { useSettings } from "../settings";
import {
  clampPercent,
  cn,
  countLines,
  downloadText,
  fileNameWithoutExtension,
  formatBytes,
  formatMs,
  formatNumber,
  formatPercent,
  formatSignedPercent,
  inputFormatLabel,
  pluralize,
  pricingFromSettings,
  readUtf8File,
  safeDownloadName,
  strategyDescription,
  strategyLabel,
  tokenizerShortLabel,
  utf8Bytes,
  warningsForDisplay,
} from "../utils";
import { CopyButton } from "../components/CopyButton";
import { CodeBlock } from "../components/CodeBlock";
import {
  Badge,
  Button,
  EmptyState,
  ErrorNotice,
  PageHeader,
  SectionHeading,
  Select,
} from "../components/ui";

type SourceMode = "built-in" | "custom";

interface UiError {
  title: string;
  message: string;
}

function evidenceSummary(result: BenchmarkStrategyResult) {
  const total = result.critical_checks.length;
  const passed = result.critical_checks.filter((check) => check.passed).length;
  return { total, passed };
}

function strategyBarClass(strategy: Strategy): string {
  if (strategy === "conservative") return "bg-ink-soft";
  if (strategy === "balanced") return "bg-rust";
  return "bg-ink";
}

function BenchmarkBars({ run }: { run: BenchmarkRun }) {
  return (
    <div className="border border-line bg-surface p-4 sm:p-5">
      <SectionHeading
        title="Reduction profile"
        description="Measured byte and token reduction vs. the original. This is not a recommendation."
      />
      <div className="space-y-5">
        {run.results.map((result) => {
          const metrics = result.compression.metrics;
          return (
            <div key={result.strategy} className="grid gap-2 sm:grid-cols-[110px_1fr] sm:gap-4">
              <span className="pt-0.5 text-[12px] font-medium">{strategyLabel(result.strategy)}</span>
              <div className="space-y-2">
                <div>
                  <div className="mb-1 flex items-[baseline] justify-between gap-2 font-mono text-[9.5px] text-muted">
                    <span>Bytes</span>
                    <span className="text-ink">{formatPercent(metrics.byte_reduction_percent)}</span>
                  </div>
                  <div className="h-2 overflow-hidden border border-line bg-paper-deep/50">
                    <div
                      aria-hidden="true"
                      className={cn("meter-grow h-full", strategyBarClass(result.strategy))}
                      style={{ width: `${clampPercent(metrics.byte_reduction_percent)}%` }}
                    />
                  </div>
                </div>
                <div>
                  <div className="mb-1 flex items-[baseline] justify-between gap-2 font-mono text-[9.5px] text-muted">
                    <span>Tokens</span>
                    <span className="text-ink">{formatPercent(metrics.token_reduction_percent)}</span>
                  </div>
                  <div className="h-1 overflow-hidden border border-line bg-paper-deep/50">
                    <div
                      aria-hidden="true"
                      className={cn("meter-grow h-full opacity-60", strategyBarClass(result.strategy))}
                      style={{
                        width: `${clampPercent(metrics.token_reduction_percent)}%`,
                        animationDelay: "80ms",
                      }}
                    />
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
      <p className="mt-5 border-t border-line pt-3 font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
        All strategies consume the same original input; output size varies by strategy.
      </p>
    </div>
  );
}

function EvidenceChecks({ result }: { result: BenchmarkStrategyResult }) {
  const { total, passed } = evidenceSummary(result);
  return (
    <div>
      <SectionHeading
        title="Manifest evidence"
        description="Explicit substring assertions from the fixture manifest. These are not semantic proofs."
        actions={
          total === 0 ? (
            <Badge>Custom input</Badge>
          ) : (
            <Badge tone={passed === total ? "success" : "danger"}>
              {passed}/{total} passed
            </Badge>
          )
        }
      />
      {total === 0 ? (
        <p className="border-y border-line py-4 text-[12.5px] leading-5 text-muted">
          Custom and pasted inputs have no manifest-declared evidence assertions. Preservation
          checks still ran, and every strategy result reports warnings and omissions.
        </p>
      ) : (
        <ul className="divide-y divide-line border-y border-line">
          {result.critical_checks.map((check) => (
            <li key={check.pattern} className="flex items-start gap-3 py-3">
              {check.passed ? (
                <CheckCircle2 aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-moss" />
              ) : (
                <TriangleAlert aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-danger" />
              )}
              <div className="min-w-0 flex-1">
                <code className="block overflow-wrap-anywhere rounded-[2px] bg-paper-deep/55 px-1.5 py-0.5 font-mono text-[11px] text-ink">
                  {check.pattern}
                </code>
                <p className="mt-1 text-[11.5px] leading-4 text-muted">{check.detail}</p>
              </div>
              <Badge tone={check.passed ? "success" : "danger"}>
                {check.passed ? "retained" : "missing"}
              </Badge>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function SelectedStrategyDetail({ result }: { result: BenchmarkStrategyResult }) {
  const compression = result.compression;
  const warnings = warningsForDisplay(compression.warnings);
  const totalOmitted =
    compression.omissions.repeated_lines_omitted +
    compression.omissions.passing_test_lines_omitted +
    compression.omissions.progress_lines_omitted +
    compression.omissions.normalized_log_lines_omitted +
    compression.omissions.json_duplicate_items_omitted;
  const omissions = [
    ["Repeated lines", compression.omissions.repeated_lines_omitted],
    ["Passing-test lines", compression.omissions.passing_test_lines_omitted],
    ["Progress updates", compression.omissions.progress_lines_omitted],
    ["Normalized log lines", compression.omissions.normalized_log_lines_omitted],
    ["Duplicate JSON records", compression.omissions.json_duplicate_items_omitted],
  ].filter((entry): entry is [string, number] => typeof entry[1] === "number" && entry[1] > 0);

  return (
    <div className="space-y-6">
      <div className="grid gap-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4">
        {[
          {
            label: "Output bytes",
            value: formatNumber(compression.metrics.output_bytes),
            detail: `${formatBytes(compression.metrics.original_bytes)} → ${formatBytes(
              compression.metrics.output_bytes,
            )}`,
          },
          {
            label: "Output tokens",
            value: formatNumber(compression.metrics.output_tokens),
            detail: `${compression.metrics.token_measurement.name} · ${
              compression.metrics.token_measurement.kind
            }`,
          },
          {
            label: "Omitted spans",
            value: formatNumber(totalOmitted),
            detail: pluralize(compression.omissions.markers_inserted, "marker"),
          },
          {
            label: "Transform time",
            value: formatMs(compression.metrics.processing_time_ms),
            detail: "Tokenization excluded",
          },
        ].map((stat) => (
          <div key={stat.label} className="bg-surface px-4 py-4">
            <p className="font-mono text-[9px] uppercase tracking-[0.12em] text-muted">{stat.label}</p>
            <p className="mt-1.5 font-display text-[2rem] leading-none text-ink">{stat.value}</p>
            <p className="mt-1.5 font-mono text-[9px] leading-4 text-muted">{stat.detail}</p>
          </div>
        ))}
      </div>

      <div
        className={cn(
          "flex flex-wrap items-start justify-between gap-3 border px-4 py-3.5",
          compression.preservation.passed
            ? "border-moss/25 bg-moss-soft/55"
            : "border-danger/30 bg-danger-soft/55",
        )}
      >
        <div className="flex items-start gap-3">
          <ShieldCheck
            aria-hidden="true"
            className={cn(
              "mt-0.5 h-4 w-4",
              compression.preservation.passed ? "text-moss" : "text-danger",
            )}
          />
          <div>
            <p className="text-[13px] font-semibold text-ink">
              {compression.preservation.passed
                ? "Preservation validation passed"
                : "Preservation validation failed"}
            </p>
            <p className="mt-0.5 text-[12px] leading-5 text-muted">
              {formatNumber(compression.preservation.retained_occurrences)} of{" "}
              {formatNumber(compression.preservation.protected_occurrences)} protected occurrences
              retained.
            </p>
          </div>
        </div>
        <Badge tone={compression.fallback_used ? "warning" : "success"}>
          {compression.fallback_used ? "Original returned" : "Transformation accepted"}
        </Badge>
      </div>

      {compression.fallback_used ? (
        <p className="border border-amber/30 bg-amber-soft/55 px-4 py-3 text-[12px] leading-5 text-amber-dark">
          <strong>Fallback used.</strong>{" "}
          {compression.fallback_reason ?? "The candidate was not smaller than the original input."}
        </p>
      ) : null}

      <EvidenceChecks result={result} />

      <div className="grid gap-6 lg:grid-cols-2">
        <div>
          <SectionHeading title="Omission vocabulary" />
          {omissions.length === 0 ? (
            <p className="border-y border-line py-4 text-[12.5px] text-muted">
              No omission categories were applied for this strategy.
            </p>
          ) : (
            <dl className="divide-y divide-line border-y border-line">
              {omissions.map(([label, count]) => (
                <div key={label} className="flex items-center justify-between gap-4 py-2">
                  <dt className="text-[12px] text-muted">{label}</dt>
                  <dd className="font-mono text-[12px]">{formatNumber(count)}</dd>
                </div>
              ))}
            </dl>
          )}
        </div>
        <div>
          <SectionHeading title="Warnings" />
          {warnings.length === 0 ? (
            <p className="border-y border-line py-4 text-[12.5px] text-muted">
              No warnings were emitted for this strategy.
            </p>
          ) : (
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
          )}
        </div>
      </div>
    </div>
  );
}

function BenchmarkResults({ run }: { run: BenchmarkRun }) {
  const [selectedStrategy, setSelectedStrategy] = useState<Strategy>("balanced");
  const selected =
    run.results.find((result) => result.strategy === selectedStrategy) ?? run.results[0];
  const original = run.results[0]?.compression.metrics.original_bytes ?? run.case.original_bytes;
  const allEvidence = run.results.every((result) =>
    result.critical_checks.every((check) => check.passed),
  );
  const assertionTotal = run.results[0]?.critical_checks.length ?? 0;

  if (!selected) {
    return null;
  }

  return (
    <div className="space-y-5">
      <div className="border border-line bg-surface">
        <div className="flex flex-wrap items-start justify-between gap-4 border-b border-line p-4 sm:p-5">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone="accent">{run.case.source === "built-in" ? "Built-in fixture" : "Custom input"}</Badge>
              <Badge>{inputFormatLabel(run.case.format)}</Badge>
              <Badge tone={allEvidence ? "success" : "danger"}>
                {assertionTotal === 0
                  ? "No manifest assertions"
                  : allEvidence
                    ? "All evidence retained"
                    : "Evidence failure"}
              </Badge>
            </div>
            <h2 className="mt-3 font-display text-[2rem] leading-none tracking-[-0.02em]">
              {run.case.name}
            </h2>
            <p className="mt-2 max-w-2xl text-[12.5px] leading-5 text-muted">
              {run.case.description}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {run.case.source === "built-in" ? (
              <CopyButton
                value="contextdrop benchmark fixtures --format text"
                label="Copy CLI command"
                copiedLabel="CLI command copied"
              />
            ) : null}
            <Button
              variant="secondary"
              size="sm"
              onClick={() =>
                downloadText(
                  `${safeDownloadName(run.case.name)}-benchmark.json`,
                  JSON.stringify(run, null, 2),
                  "application/json;charset=utf-8",
                )
              }
            >
              <Download aria-hidden="true" className="h-3.5 w-3.5" />
              Download JSON
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-x-6 gap-y-2 border-b border-line px-4 py-2.5 font-mono text-[9.5px] uppercase tracking-[0.08em] text-muted sm:px-5">
          <span>
            Original <span className="text-ink">{formatBytes(original)}</span>
          </span>
          <span>
            Tokenizer{" "}
            <span className="text-ink">{selected.compression.metrics.token_measurement.name}</span>
          </span>
          <span>
            Strategies <span className="text-ink">3</span>
          </span>
          <span className="ml-auto normal-case tracking-normal">
            times are transformation-only
          </span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full min-w-[820px] border-collapse text-left">
            <caption className="sr-only">
              Compression benchmark results for all three strategies. Select a row to view detailed
              evidence.
            </caption>
            <thead>
              <tr className="border-b border-line bg-paper/50 font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
                <th scope="col" className="px-4 py-3 font-normal sm:px-5">
                  Strategy
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Output bytes
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Reduction
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Output tokens
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Omitted
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Time
                </th>
                <th scope="col" className="px-3 py-3 font-normal">
                  Preserve
                </th>
                <th scope="col" className="px-4 py-3 font-normal sm:px-5">
                  Evidence
                </th>
              </tr>
            </thead>
            <tbody>
              {run.results.map((result) => {
                const metrics = result.compression.metrics;
                const evidence = evidenceSummary(result);
                const active = result.strategy === selected.strategy;
                return (
                  <tr
                    key={result.strategy}
                    className={cn(
                      "border-b border-line/75 text-[12px] transition-colors last:border-0",
                      active ? "bg-rust-soft/25" : "bg-surface hover:bg-paper/60",
                    )}
                  >
                    <th scope="row" className="px-4 py-3 sm:px-5">
                      <button
                        type="button"
                        onClick={() => setSelectedStrategy(result.strategy)}
                        aria-pressed={active}
                        aria-label={`View ${strategyLabel(result.strategy)} details`}
                        className="flex items-center gap-2 text-left font-medium text-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
                      >
                        <span
                          aria-hidden="true"
                          className={cn(
                            "grid h-3.5 w-3.5 place-items-center rounded-full border",
                            active ? "border-rust bg-rust" : "border-line-strong bg-surface",
                          )}
                        >
                          {active ? <span className="h-1 w-1 rounded-full bg-white" /> : null}
                        </span>
                        <span>
                          {strategyLabel(result.strategy)}
                          <span className="mt-0.5 block font-mono text-[8.5px] font-normal uppercase tracking-[0.08em] text-muted">
                            {active ? "viewing details" : "select"}
                          </span>
                        </span>
                      </button>
                    </th>
                    <td className="whitespace-nowrap px-3 py-3 font-mono">
                      {formatNumber(metrics.output_bytes)}
                      <span className="ml-1 text-[10px] text-muted">B</span>
                    </td>
                    <td className="whitespace-nowrap px-3 py-3 font-mono text-moss">
                      {formatSignedPercent(metrics.byte_reduction_percent)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-3 font-mono">
                      {formatNumber(metrics.output_tokens)}
                      <span className="mt-0.5 block text-[9.5px] font-normal text-muted">
                        {formatSignedPercent(metrics.token_reduction_percent)}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-3 py-3 font-mono">
                      {formatNumber(
                        result.compression.omissions.repeated_lines_omitted +
                          result.compression.omissions.passing_test_lines_omitted +
                          result.compression.omissions.progress_lines_omitted +
                          result.compression.omissions.normalized_log_lines_omitted +
                          result.compression.omissions.json_duplicate_items_omitted,
                      )}
                    </td>
                    <td className="whitespace-nowrap px-3 py-3 font-mono text-muted">
                      {formatMs(metrics.processing_time_ms)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-3">
                      <Badge tone={result.compression.preservation.passed ? "success" : "danger"}>
                        {result.compression.preservation.passed ? "pass" : "fail"}
                      </Badge>
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 sm:px-5">
                      {evidence.total === 0 ? (
                        <span className="font-mono text-[10px] text-muted">none</span>
                      ) : (
                        <Badge tone={evidence.passed === evidence.total ? "success" : "danger"}>
                          {evidence.passed}/{evidence.total}
                        </Badge>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <BenchmarkBars run={run} />

      <div className="border border-line bg-surface p-4 sm:p-5">
        <div className="mb-5 flex flex-wrap items-start justify-between gap-3 border-b border-line pb-4">
          <div>
            <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-muted">
              Selected strategy
            </p>
            <h3 className="mt-1 font-display text-3xl leading-none">
              {strategyLabel(selected.strategy)}
            </h3>
            <p className="mt-1.5 max-w-xl text-[12px] leading-5 text-muted">
              {strategyDescription(selected.strategy)}
            </p>
          </div>
          <div className="flex flex-wrap gap-1">
            {run.results.map((result) => (
              <button
                key={result.strategy}
                type="button"
                aria-pressed={result.strategy === selected.strategy}
                onClick={() => setSelectedStrategy(result.strategy)}
                className={cn(
                  "min-h-9 border px-2.5 text-[11px] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust",
                  result.strategy === selected.strategy
                    ? "border-ink bg-ink text-paper"
                    : "border-line-strong bg-surface text-ink-soft hover:border-ink",
                )}
              >
                {strategyLabel(result.strategy)}
              </button>
            ))}
          </div>
        </div>
        <SelectedStrategyDetail result={selected} />
      </div>
    </div>
  );
}

export function BenchmarkPage() {
  const { settings } = useSettings();
  const [mode, setMode] = useState<SourceMode>("built-in");
  const [fixtures, setFixtures] = useState<FixtureSummary[]>([]);
  const [fixturesError, setFixturesError] = useState<string | null>(null);
  const [fixtureAttempt, setFixtureAttempt] = useState(0);
  const [selectedFixtureId, setSelectedFixtureId] = useState("");
  const [fixtureDetail, setFixtureDetail] = useState<FixtureDetail | null>(null);
  const [fixtureLoading, setFixtureLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [customText, setCustomText] = useState("");
  const [customName, setCustomName] = useState("Pasted input");
  const [customFormat, setCustomFormat] = useState<InputFormat>("auto");
  const [tokenizer, setTokenizer] = useState<TokenizerKind>(settings.tokenizer);
  const [run, setRun] = useState<BenchmarkRun | null>(null);
  const [runAt, setRunAt] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [dragActive, setDragActive] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const runAbort = useRef<AbortController | null>(null);
  const detailAbort = useRef<AbortController | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setFixturesError(null);
    getFixtures(controller.signal)
      .then((data) => {
        setFixtures(data);
        setSelectedFixtureId((current) => {
          if (current && data.some((fixture) => fixture.id === current)) {
            return current;
          }
          return (
            data.find((fixture) => fixture.id === "rust-build-failure")?.id ?? data[0]?.id ?? ""
          );
        });
      })
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setFixturesError(errorMessage(caught));
        }
      });
    return () => controller.abort();
  }, [fixtureAttempt]);

  useEffect(() => {
    if (mode !== "built-in" || !selectedFixtureId) {
      setFixtureDetail(null);
      setDetailError(null);
      setFixtureLoading(false);
      return;
    }
    detailAbort.current?.abort();
    const controller = new AbortController();
    detailAbort.current = controller;
    setFixtureLoading(true);
    setDetailError(null);
    getFixture(selectedFixtureId, controller.signal)
      .then(setFixtureDetail)
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setFixtureDetail(null);
          setDetailError(errorMessage(caught));
        }
      })
      .finally(() => {
        if (detailAbort.current === controller) {
          detailAbort.current = null;
          setFixtureLoading(false);
        }
      });
    return () => controller.abort();
  }, [mode, selectedFixtureId]);

  useEffect(() => {
    return () => {
      runAbort.current?.abort();
      detailAbort.current?.abort();
    };
  }, []);

  const customBytes = useMemo(() => utf8Bytes(customText), [customText]);
  const customLines = useMemo(() => countLines(customText), [customText]);
  const overCustomLimit = customBytes > settings.maxInputBytes;
  const overFixtureLimit =
    mode === "built-in" &&
    fixtureDetail !== null &&
    fixtureDetail.fixture.original_bytes > settings.maxInputBytes;

  const sourceReady =
    mode === "built-in" ? selectedFixtureId.length > 0 && !overFixtureLimit : customText.length > 0;
  const canRun = sourceReady && !busy && !(mode === "custom" && overCustomLimit);

  const groupedFixtures = useMemo(() => {
    const groups = new Map<string, FixtureSummary[]>();
    for (const fixture of fixtures) {
      const list = groups.get(fixture.category) ?? [];
      list.push(fixture);
      groups.set(fixture.category, list);
    }
    return [...groups.entries()];
  }, [fixtures]);

  const executeBenchmark = useCallback(async () => {
    if (!canRun) {
      return;
    }
    runAbort.current?.abort();
    const controller = new AbortController();
    runAbort.current = controller;
    setBusy(true);
    setError(null);
    const pricing = pricingFromSettings(
      settings.pricingEnabled,
      settings.pricePerMillion,
      settings.currency,
    );
    const base = {
      tokenizer,
      preserve_patterns: settings.preservePatterns,
      ...(pricing ? { pricing } : {}),
      collapse_repetitions: settings.collapseRepetitions,
      collapse_passing_tests: settings.collapsePassingTests,
      collapse_progress_updates: settings.collapseProgressUpdates,
      structural_json: settings.structuralJson,
      max_input_bytes: settings.maxInputBytes,
    };
    const payload: BenchmarkRequest =
      mode === "built-in"
        ? {
            ...base,
            fixture_id: selectedFixtureId,
            input_format: "auto",
          }
        : {
            ...base,
            text: customText,
            name: customName.trim() || "Pasted input",
            input_format: customFormat,
          };
    try {
      const benchmark = await runBenchmark(payload, controller.signal);
      setRun(benchmark);
      setRunAt(Date.now());
    } catch (caught: unknown) {
      if (!isAbortError(caught)) {
        setError({ title: "Benchmark failed", message: errorMessage(caught) });
      }
    } finally {
      if (runAbort.current === controller) {
        runAbort.current = null;
        setBusy(false);
      }
    }
  }, [canRun, customFormat, customName, customText, mode, selectedFixtureId, settings, tokenizer]);

  const readCustomFile = useCallback(async (files: FileList | null) => {
    const file = files?.[0];
    if (!file) {
      return;
    }
    setError(null);
    if (file.size > settings.maxInputBytes) {
      setError({
        title: "File is too large",
        message: `This file is ${formatBytes(file.size)}; the current limit is ${formatBytes(
          settings.maxInputBytes,
        )}. Raise the limit in Settings.`,
      });
      return;
    }
    try {
      const decoded = await readUtf8File(file);
      if (utf8Bytes(decoded) > settings.maxInputBytes) {
        setError({
          title: "File is too large",
          message: `The decoded input exceeds the ${formatBytes(settings.maxInputBytes)} limit.`,
        });
        return;
      }
      setCustomText(decoded);
      setCustomName(fileNameWithoutExtension(file.name));
      setMode("custom");
      setRun(null);
      setDragActive(false);
    } catch {
      setError({
        title: "Could not read file",
        message: "The selected file is not valid UTF-8 text or could not be read by this browser.",
      });
    }
  }, [settings.maxInputBytes]);

  function switchMode(next: SourceMode) {
    if (next === mode) return;
    setMode(next);
    setRun(null);
    setError(null);
  }

  const evidenceAll = run
    ? run.results.every((result) => result.critical_checks.every((check) => check.passed))
    : false;
  const fixtureSelected = fixtures.find((fixture) => fixture.id === selectedFixtureId);

  return (
    <>
      <PageHeader
        index="02 / Benchmark Lab"
        title="Benchmark Lab"
        description="Run the real compression engine with all three strategies against labeled failure fixtures or your own context. Inspect every savings claim and every evidence check."
        actions={
          <Link
            to="/workspace"
            className="inline-flex min-h-10 items-center gap-2 border border-line-strong bg-surface px-3.5 py-2 text-[13px] font-medium text-ink transition-colors hover:border-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
          >
            Open Workspace
            <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
          </Link>
        }
        meta={
          <>
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Strategies <span className="text-ink">3</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Fixtures <span className="text-ink">{fixtures.length || "—"}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Tokenizer <span className="text-ink">{tokenizerShortLabel(tokenizer)}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Rules{" "}
              <span className="text-ink">
                {settings.preservePatterns.length > 0 ? `${settings.preservePatterns.length} custom` : "built-in"}
              </span>
            </span>
            <Link
              to="/settings"
              className="ml-auto font-mono text-[9px] uppercase tracking-[0.1em] text-rust hover:text-rust-dark hover:underline"
            >
              Edit run defaults
            </Link>
          </>
        }
      />

      <div className="sr-only" role="status" aria-live="polite">
        {busy
          ? "Benchmark running locally across all three strategies."
          : run
            ? `Benchmark complete. ${evidenceAll ? "All manifest evidence retained." : "One or more evidence checks failed."}`
            : ""}
      </div>

      <section aria-labelledby="source-heading" className="border border-line bg-surface">
        <div className="grid lg:grid-cols-[minmax(0,1fr)_290px]">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line p-3 sm:p-4">
              <h2
                id="source-heading"
                className="font-mono text-[10px] font-medium uppercase tracking-[0.14em]"
              >
                01 / Source
              </h2>
              <div role="group" aria-label="Benchmark source" className="flex border border-line-strong p-0.5">
                {(
                  [
                    ["built-in", "Built-in fixture"],
                    ["custom", "Paste / file"],
                  ] as Array<[SourceMode, string]>
                ).map(([value, label]) => (
                  <button
                    key={value}
                    type="button"
                    aria-pressed={mode === value}
                    disabled={busy}
                    onClick={() => switchMode(value)}
                    className={cn(
                      "min-h-8 px-2.5 text-[11px] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust disabled:opacity-50",
                      mode === value ? "bg-ink text-paper" : "text-ink-soft hover:bg-paper-deep/60",
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {mode === "built-in" ? (
              <div className="p-3 sm:p-4">
                <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
                  <Select
                    label="Failure fixture"
                    value={selectedFixtureId}
                    onChange={(event) => {
                      setSelectedFixtureId(event.target.value);
                      setRun(null);
                      setError(null);
                    }}
                    disabled={busy || fixtures.length === 0}
                    hint="Synthetic, deterministic fixtures stored in the repository."
                  >
                    {fixtures.length === 0 ? <option value="">Loading fixtures…</option> : null}
                    {groupedFixtures.map(([category, entries]) => (
                      <optgroup key={category} label={category}>
                        {entries.map((fixture) => (
                          <option key={fixture.id} value={fixture.id}>
                            {fixture.name}
                          </option>
                        ))}
                      </optgroup>
                    ))}
                  </Select>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => fileInputRef.current?.click()}
                    disabled={busy}
                    className="mb-0.5"
                  >
                    <Upload aria-hidden="true" className="h-3.5 w-3.5" />
                    Open local file
                  </Button>
                </div>

                {fixturesError ? (
                  <div className="mt-3">
                    <ErrorNotice
                      title="Fixtures unavailable"
                      message={fixturesError}
                      onRetry={() => setFixtureAttempt((attempt) => attempt + 1)}
                    />
                  </div>
                ) : null}
                {detailError ? (
                  <div className="mt-3">
                    <ErrorNotice title="Fixture preview failed" message={detailError} />
                  </div>
                ) : null}

                {fixtureSelected ? (
                  <div className="mt-4">
                    <div className="flex flex-wrap items-start justify-between gap-3 border border-line bg-paper/45 px-3.5 py-3">
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge tone="accent">{fixtureSelected.category}</Badge>
                          <Badge>{inputFormatLabel(fixtureSelected.format)}</Badge>
                          <span className="font-mono text-[10px] text-muted">
                            {formatBytes(fixtureSelected.original_bytes)}
                          </span>
                          <span className="font-mono text-[10px] text-muted">
                            {pluralize(fixtureSelected.assertion_count, "assertion")}
                          </span>
                        </div>
                        <p className="mt-2 max-w-2xl text-[12.5px] leading-5 text-ink-soft">
                          {fixtureSelected.description}
                        </p>
                        {fixtureSelected.notes ? (
                          <p className="mt-2 max-w-2xl border-l-2 border-rust/40 pl-2.5 text-[11.5px] leading-5 text-muted">
                            {fixtureSelected.notes}
                          </p>
                        ) : null}
                      </div>
                    </div>

                    {fixtureLoading ? (
                      <div className="skeleton mt-3 h-56 border border-line" aria-label="Loading fixture preview" />
                    ) : fixtureDetail ? (
                      <div className="mt-3">
                        <CodeBlock
                          value={fixtureDetail.content}
                          ariaLabel={`Preview of ${fixtureDetail.fixture.name}`}
                          maxHeight="17rem"
                          header={
                            <>
                              <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-code-muted">
                                source preview
                              </span>
                              <CopyButton
                                value={fixtureDetail.content}
                                label="Copy source"
                                compact
                                className="text-code-text hover:bg-code-line"
                              />
                            </>
                          }
                          footer={`${formatBytes(fixtureDetail.fixture.original_bytes)} · ${pluralize(
                            countLines(fixtureDetail.content),
                            "line",
                          )} · embedded locally`}
                        />
                      </div>
                    ) : null}
                  </div>
                ) : null}

                {overFixtureLimit ? (
                  <p className="mt-3 border border-danger/25 bg-danger-soft/50 px-3 py-2 text-[12px] text-danger">
                    This fixture exceeds the current input limit. Raise it in Settings.
                  </p>
                ) : null}
              </div>
            ) : (
              <div className="p-3 sm:p-4">
                <div
                  className={cn(
                    "relative",
                    dragActive && "ring-2 ring-rust ring-offset-2 ring-offset-surface",
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
                    void readCustomFile(event.dataTransfer.files);
                  }}
                >
                  <textarea
                    value={customText}
                    onChange={(event) => {
                      setCustomText(event.target.value);
                      setRun(null);
                      setError(null);
                    }}
                    disabled={busy}
                    aria-label="Custom benchmark input"
                    spellCheck={false}
                    placeholder="Paste test output, a build failure, structured logs, or JSON…"
                    className="field-input field-textarea min-h-[15rem] w-full"
                  />
                  {dragActive ? (
                    <div className="pointer-events-none absolute inset-0 grid place-items-center border border-rust bg-paper/90">
                      <div className="text-center">
                        <Upload aria-hidden="true" className="mx-auto h-5 w-5 text-rust" />
                        <p className="mt-2 text-sm font-medium">Drop a UTF-8 text file</p>
                      </div>
                    </div>
                  ) : null}
                </div>
                <div className="mt-3 grid gap-3 sm:grid-cols-2">
                  <label className="block">
                    <span className="mb-1.5 block font-mono text-[10px] font-medium uppercase tracking-[0.12em] text-ink-soft">
                      Input name
                    </span>
                    <input
                      type="text"
                      value={customName}
                      onChange={(event) => {
                        setCustomName(event.target.value.slice(0, 120));
                        setRun(null);
                      }}
                      maxLength={120}
                      disabled={busy}
                      className="field-input h-10 w-full"
                      placeholder="Pasted input"
                    />
                  </label>
                  <Select
                    label="Input format"
                    value={customFormat}
                    disabled={busy}
                    onChange={(event) => {
                      setCustomFormat(event.target.value as InputFormat);
                      setRun(null);
                    }}
                  >
                    <option value="auto">Auto-detect</option>
                    <option value="plain-text">Plain text</option>
                    <option value="terminal">Terminal</option>
                    <option value="build-log">Build log</option>
                    <option value="test-output">Test output</option>
                    <option value="json">JSON</option>
                    <option value="structured-log">Structured log</option>
                  </Select>
                </div>
                <div className="mt-3 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-3">
                  <p className="font-mono text-[9.5px] text-muted">
                    {formatNumber(customBytes)} bytes · {pluralize(customLines, "line")} ·{" "}
                    {formatBytes(settings.maxInputBytes)} limit
                  </p>
                  <Button variant="ghost" size="sm" onClick={() => fileInputRef.current?.click()}>
                    <Upload aria-hidden="true" className="h-3.5 w-3.5" />
                    Open local file
                  </Button>
                </div>
                {overCustomLimit ? (
                  <p className="mt-2 border border-danger/25 bg-danger-soft/50 px-3 py-2 text-[12px] text-danger">
                    Input exceeds the current limit by{" "}
                    {formatBytes(customBytes - settings.maxInputBytes)}. Raise it in Settings or
                    reduce the input.
                  </p>
                ) : null}
              </div>
            )}
          </div>

          <aside className="border-t border-line bg-paper/40 p-4 lg:border-l lg:border-t-0 lg:p-5">
            <h2 className="font-mono text-[10px] font-medium uppercase tracking-[0.14em]">
              Run setup
            </h2>
            <p className="mt-2 text-[12px] leading-5 text-muted">
              Each run uses the real engine on this machine. Results are never cached or simulated.
            </p>

            <div className="mt-4 border-y border-line py-3">
              <div className="flex items-center justify-between gap-2 text-[12px]">
                <span className="text-muted">Strategies</span>
                <span className="font-medium">All three</span>
              </div>
              <div className="mt-2 flex gap-1">
                {(["conservative", "balanced", "aggressive"] as Strategy[]).map((strategy) => (
                  <span
                    key={strategy}
                    aria-hidden="true"
                    className="h-1 flex-1 bg-line-strong first:bg-ink-soft last:bg-ink"
                  />
                ))}
              </div>
              <p className="mt-2 font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
                Safe → balanced → deep
              </p>
            </div>

            <div className="mt-4 space-y-3">
              <Select
                label="Token measurement"
                value={tokenizer}
                disabled={busy}
                onChange={(event) => {
                  setTokenizer(event.target.value as TokenizerKind);
                  setRun(null);
                }}
                hint="Exact tokenization is local and may initialize a vocabulary on first use."
              >
                <option value="o200k_base">o200k_base · exact</option>
                <option value="cl100k_base">cl100k_base · exact</option>
                <option value="estimate">Heuristic estimate</option>
              </Select>
              <dl className="divide-y divide-line border-y border-line text-[11px]">
                <div className="flex items-center justify-between gap-2 py-1.5">
                  <dt className="text-muted">Input limit</dt>
                  <dd className="font-mono">{formatBytes(settings.maxInputBytes)}</dd>
                </div>
                <div className="flex items-center justify-between gap-2 py-1.5">
                  <dt className="text-muted">Custom rules</dt>
                  <dd className="font-mono">{settings.preservePatterns.length}</dd>
                </div>
                <div className="flex items-center justify-between gap-2 py-1.5">
                  <dt className="text-muted">Cost signal</dt>
                  <dd className="font-mono">{settings.pricingEnabled ? "on" : "off"}</dd>
                </div>
              </dl>
              <p className="text-[11px] leading-5 text-muted">
                Preserve rules, transform toggles, and limits come from Settings.{" "}
                <Link to="/settings" className="text-rust underline decoration-rust/30 hover:decoration-rust">
                  Edit
                </Link>
              </p>
            </div>

            <Button
              variant="primary"
              className="mt-5 w-full justify-center"
              loading={busy}
              disabled={!canRun}
              onClick={() => void executeBenchmark()}
            >
              {busy ? "Running locally…" : "Run benchmark"}
              {!busy ? <ArrowRight aria-hidden="true" className="h-4 w-4" /> : null}
            </Button>
            <p className="mt-2 text-center font-mono text-[8.5px] uppercase tracking-[0.1em] text-muted">
              no network calls beyond loopback
            </p>
          </aside>
        </div>
      </section>

      <input
        ref={fileInputRef}
        type="file"
        accept=".log,.txt,.json,.jsonl,.ndjson,.out,.trace,.md"
        onChange={(event) => {
          const files = event.currentTarget.files;
          void readCustomFile(files);
          event.currentTarget.value = "";
        }}
      />

      {error ? (
        <div className="mt-3">
          <ErrorNotice
            title={error.title}
            message={error.message}
            onRetry={error.title === "Benchmark failed" && canRun ? () => void executeBenchmark() : undefined}
          />
        </div>
      ) : null}

      <div className="mt-8">
        <div className="mb-4 flex flex-wrap items-end justify-between gap-3 border-b border-line pb-3">
          <div>
            <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-muted">
              02 / Measured results
            </p>
            <h2 className="mt-1 font-display text-4xl leading-none tracking-[-0.02em]">
              Evidence over ratios.
            </h2>
          </div>
          {run && runAt ? (
            <p className="font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
              measured {new Date(runAt).toLocaleTimeString()}
            </p>
          ) : null}
        </div>

        {busy ? (
          <div
            className="border border-line bg-surface p-4 sm:p-5"
            role="status"
            aria-live="polite"
          >
            <div className="flex items-center gap-3">
              <Gauge aria-hidden="true" className="h-4 w-4 animate-pulse text-rust" />
              <p className="text-sm font-medium">Running all three strategies locally…</p>
            </div>
            <p className="mt-1.5 text-[12px] leading-5 text-muted">
              The engine is processing the source and validating protected evidence. This dashboard
              does not invent progress percentages.
            </p>
            <div className="mt-4 space-y-2">
              <div className="skeleton h-9 w-full" />
              <div className="skeleton h-9 w-full" style={{ animationDelay: "120ms" }} />
              <div className="skeleton h-9 w-2/3" style={{ animationDelay: "220ms" }} />
            </div>
          </div>
        ) : run ? (
          <BenchmarkResults run={run} />
        ) : (
          <div className="border border-line bg-surface empty-grid">
            <EmptyState
              icon={Gauge}
              title="A real measurement, not a dashboard sample."
              description="Choose a built-in failure fixture or paste your own context, then run all three strategies. ContextDrop will show actual byte and token reductions, evidence checks, omissions, warnings, and timings from this machine."
            >
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  if (fixtures.length === 0) {
                    setFixtureAttempt((attempt) => attempt + 1);
                  } else {
                    setMode("custom");
                  }
                }}
              >
                <FileSearch aria-hidden="true" className="h-3.5 w-3.5" />
                {fixtures.length === 0 ? "Retry fixtures" : "Use your own input"}
              </Button>
            </EmptyState>
          </div>
        )}
      </div>
    </>
  );
}
