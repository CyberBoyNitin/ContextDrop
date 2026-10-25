import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  Check,
  ListChecks,
  Plus,
  RotateCcw,
  ScanSearch,
  ShieldCheck,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { errorMessage, getFixture, getFixtures, getRules, inspectInput, isAbortError } from "../api";
import type {
  FixtureSummary,
  ProtectionInspection,
  RuleCatalog,
  RuleInfo,
} from "../types";
import { useSettings } from "../settings";
import {
  countLines,
  formatBytes,
  formatNumber,
  formatPercent,
  pluralize,
  readUtf8File,
  utf8Bytes,
} from "../utils";
import {
  Badge,
  Button,
  EmptyState,
  ErrorNotice,
  PageHeader,
  SectionHeading,
  Select,
} from "../components/ui";

const RULE_NOTES: Record<string, string> = {
  "errors-and-warnings":
    "Failure-salience words including error, fatal, panic, exception, warning, CVE identifiers, and several non-English markers.",
  "stack-trace":
    "Stack-trace frames, Python tracebacks, Rust panics, Go goroutines, cause chains, and source carets.",
  "file-and-line":
    "Source paths with a line number, including Windows and relative paths across common developer file types.",
  "python-file-and-line": "Python's File \"…\", line … reference form.",
  "exit-code": "Exit code, process status, and exited-with markers.",
  security:
    "Security, vulnerability, exploit, credential, secret, rejected-authentication, and critical-severity signals.",
  "assertion-detail":
    "Expected/Received blocks, assertion errors, snapshot notes, and failure detail lines.",
  "test-failure": "FAILED, FAIL, not ok, non-zero failed-test counts, and common failure glyphs.",
  summary: "Test and build summaries that provide the outcome of a run.",
  "http-error": "HTTP 4xx/5xx response lines and status fields.",
  "command-context":
    "Shell prompts, explicit command/argv fields, and common build or test command invocations.",
};

const STRATEGY_BOUNDARIES = [
  {
    label: "Conservative",
    removes: "Consecutive exact repeated lines outside protected evidence.",
    note: "The narrowest deterministic change. Useful when reviewing every removed line matters.",
  },
  {
    label: "Balanced",
    removes: "Passing-test runs, progress updates, and exact duplicate records in large JSON arrays.",
    note: "Adds the Conservative behavior. The default for general agent context.",
  },
  {
    label: "Aggressive",
    removes: "Global exact duplicate lines and normalized structured-log groups.",
    note: "Adds the Balanced behavior. Deepest reduction; review the diff and warnings.",
  },
];

function reasonLabel(reason: string, rules: RuleInfo[]): string {
  if (reason === "user-pattern") return "custom preserve pattern";
  const match = rules.find((rule) => rule.id === reason);
  if (match) return match.label;
  return reason.replaceAll("-", " ");
}

function PatternEditor({
  patterns,
  setPatterns,
  activePatterns,
  onApply,
  onDiscard,
  persisted,
}: {
  patterns: string[];
  setPatterns: (patterns: string[]) => void;
  activePatterns: string[];
  onApply: () => void;
  onDiscard: () => void;
  persisted: boolean;
}) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const dirty = JSON.stringify(patterns) !== JSON.stringify(activePatterns);

  function addPattern() {
    const trimmed = draft.trim();
    if (!trimmed) {
      setError("Enter a literal string to preserve.");
      return;
    }
    if (trimmed.length > 256) {
      setError("Patterns must be 256 characters or fewer.");
      return;
    }
    if (/[\r\n\0]/.test(trimmed)) {
      setError("Patterns cannot contain line breaks or null characters.");
      return;
    }
    if (patterns.some((pattern) => pattern.toLowerCase() === trimmed.toLowerCase())) {
      setError("That pattern is already in the list.");
      return;
    }
    if (patterns.length >= 100) {
      setError("At most 100 custom preserve patterns are supported.");
      return;
    }
    setPatterns([...patterns, trimmed]);
    setDraft("");
    setError(null);
    setStatus(null);
  }

  function removePattern(pattern: string) {
    setPatterns(patterns.filter((entry) => entry !== pattern));
    setError(null);
    setStatus(null);
  }

  function applyPatterns() {
    onApply();
    setStatus(
      persisted
        ? "Patterns applied to this browser. New Workspace and Benchmark runs will use them."
        : "Patterns applied for this session, but browser storage is unavailable. They will reset on reload.",
    );
  }

  return (
    <section aria-labelledby="custom-patterns-heading" className="border border-line bg-surface">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-line p-4 sm:p-5">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <h2 id="custom-patterns-heading" className="text-[15px] font-semibold">
              Custom preserve patterns
            </h2>
            <Badge tone={dirty ? "warning" : "neutral"}>
              {dirty ? "unapplied changes" : "applied"}
            </Badge>
          </div>
          <p className="mt-1.5 max-w-xl text-[12.5px] leading-5 text-muted">
            Literal, case-insensitive substrings. Any input line containing a pattern is protected
            from deterministic omission. Patterns are never treated as regex, shell syntax, or code.
          </p>
        </div>
        <span className="font-mono text-[10px] text-muted">{patterns.length}/100</span>
      </div>

      <div className="p-4 sm:p-5">
        <form
          onSubmit={(event) => {
            event.preventDefault();
            addPattern();
          }}
          className="flex flex-col gap-2 sm:flex-row"
        >
          <label className="min-w-0 flex-1">
            <span className="mb-1.5 block font-mono text-[10px] uppercase tracking-[0.12em] text-ink-soft">
              Add literal pattern
            </span>
            <input
              type="text"
              value={draft}
              maxLength={256}
              disabled={patterns.length >= 100}
              onChange={(event) => {
                setDraft(event.target.value);
                setError(null);
                setStatus(null);
              }}
              className="field-input h-10 w-full"
              placeholder="e.g. tenant_id=acme or deploy-target-7"
              aria-describedby="pattern-help"
            />
          </label>
          <Button
            type="submit"
            variant="secondary"
            className="self-end"
            disabled={patterns.length >= 100 || draft.trim().length === 0}
          >
            <Plus aria-hidden="true" className="h-4 w-4" />
            Add pattern
          </Button>
        </form>
        <p id="pattern-help" className="mt-2 text-[11px] leading-5 text-muted">
          Up to 256 characters. Matching is per line. Add several specific values rather than broad
          words that would shield all evidence.
        </p>
        {error ? (
          <p role="alert" className="mt-2 border-l-2 border-danger bg-danger-soft/50 px-2.5 py-2 text-[12px] text-danger">
            {error}
          </p>
        ) : null}

        <div className="mt-4 border-y border-line">
          {patterns.length === 0 ? (
            <div className="flex items-start gap-3 py-3.5">
              <ShieldCheck aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-muted" />
              <div>
                <p className="text-[12.5px] font-medium">No custom patterns.</p>
                <p className="mt-0.5 text-[11.5px] leading-5 text-muted">
                  Built-in protection rules are still active. Add a literal value when a specific
                  identifier must survive compression.
                </p>
              </div>
            </div>
          ) : (
            <ul className="divide-y divide-line">
              {patterns.map((pattern) => (
                <li key={pattern} className="flex items-center justify-between gap-3 py-2">
                  <code className="min-w-0 break-all rounded-[2px] bg-paper-deep/50 px-1.5 py-0.5 font-mono text-[11.5px] text-ink">
                    {pattern}
                  </code>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => removePattern(pattern)}
                    aria-label={`Remove preserve pattern ${pattern}`}
                    className="shrink-0"
                  >
                    <Trash2 aria-hidden="true" className="h-3.5 w-3.5" />
                    Remove
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button variant="primary" size="sm" onClick={applyPatterns} disabled={!dirty}>
            <Check aria-hidden="true" className="h-3.5 w-3.5" />
            Apply patterns
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => {
              onDiscard();
              setError(null);
              setStatus("Draft discarded. The applied pattern list is restored.");
            }}
            disabled={!dirty}
          >
            <RotateCcw aria-hidden="true" className="h-3.5 w-3.5" />
            Discard changes
          </Button>
          <span className="font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
            applied {activePatterns.length} · draft {patterns.length}
          </span>
        </div>
        <p className="sr-only" role="status" aria-live="polite">
          {status ?? ""}
        </p>
        {status ? (
          <p className="mt-2 text-[11.5px] leading-5 text-moss">{status}</p>
        ) : null}
      </div>
    </section>
  );
}

function InspectionResult({
  inspection,
  rules,
}: {
  inspection: ProtectionInspection;
  rules: RuleInfo[];
}) {
  const percent = inspection.total_lines
    ? (inspection.protected_lines / inspection.total_lines) * 100
    : 0;
  const reasons = Object.entries(inspection.reason_counts).sort((a, b) => b[1] - a[1]);
  const maxReasonCount = reasons[0]?.[1] ?? 0;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-3 divide-x divide-line border-y border-line">
        <div className="px-3 py-3">
          <p className="font-mono text-[8.5px] uppercase tracking-[0.1em] text-muted">Protected</p>
          <p className="mt-1 font-display text-[1.8rem] leading-none">{inspection.protected_lines}</p>
        </div>
        <div className="px-3 py-3">
          <p className="font-mono text-[8.5px] uppercase tracking-[0.1em] text-muted">Total lines</p>
          <p className="mt-1 font-display text-[1.8rem] leading-none">{inspection.total_lines}</p>
        </div>
        <div className="px-3 py-3">
          <p className="font-mono text-[8.5px] uppercase tracking-[0.1em] text-muted">Coverage</p>
          <p className="mt-1 font-display text-[1.8rem] leading-none">{formatPercent(percent)}</p>
        </div>
      </div>

      <div>
        <SectionHeading title="Reason associations" description="A line can match more than one rule." />
        {reasons.length === 0 ? (
          <p className="border-y border-line py-3 text-[12px] text-muted">
            No protection rules or patterns matched this input.
          </p>
        ) : (
          <div className="space-y-2">
            {reasons.map(([reason, count]) => (
              <div key={reason} className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3">
                <div>
                  <p className="text-[11.5px] leading-4 text-ink-soft">{reasonLabel(reason, rules)}</p>
                  <div className="mt-1 h-1.5 overflow-hidden bg-paper-deep">
                    <div
                      aria-hidden="true"
                      className="h-full bg-moss"
                      style={{ width: `${maxReasonCount ? (count / maxReasonCount) * 100 : 0}%` }}
                    />
                  </div>
                </div>
                <span className="font-mono text-[11px] text-ink">{count}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div>
        <SectionHeading
          title="Protected line map"
          description={`${pluralize(inspection.matches.length, "line")} shown in this preview.`}
        />
        {inspection.matches.length === 0 ? (
          <div className="border-y border-line py-4 text-[12px] text-muted">
            Nothing matched. That is a useful result: the line would be eligible for a
            strategy-specific transformation.
          </div>
        ) : (
          <ul
            tabIndex={0}
            aria-label="Protected line matches"
            className="code-scroll max-h-[24rem] divide-y divide-code-line overflow-auto border border-code-line bg-code focus-visible:outline focus-visible:outline-2 focus-visible:outline-rust"
          >
            {inspection.matches.map((match) => (
              <li key={`${match.line_number}-${match.text}`} className="flex gap-3 px-3 py-2">
                <span className="w-8 shrink-0 select-none text-right font-mono text-[10px] leading-5 text-code-muted">
                  {match.line_number}
                </span>
                <div className="min-w-0">
                  <code className="block whitespace-pre-wrap break-words font-mono text-[11px] leading-5 text-code-text/90">
                    {match.text || " "}
                  </code>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {match.reasons.map((reason) => (
                      <span
                        key={reason}
                        className="border border-code-line px-1.5 py-0.5 font-mono text-[8.5px] uppercase tracking-[0.06em] text-code-muted"
                      >
                        {reasonLabel(reason, rules)}
                      </span>
                    ))}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
        {inspection.truncated ? (
          <p className="mt-2 border-l-2 border-amber pl-2 text-[11px] leading-5 text-amber-dark">
            The match list is truncated for a bounded local response. Counts and coverage above are
            complete.
          </p>
        ) : null}
      </div>

      <p className="border-t border-line pt-3 text-[11px] leading-5 text-muted">{inspection.note}</p>
    </div>
  );
}

export function RulesPage() {
  const { settings, saveSettings, persisted } = useSettings();
  const [rules, setRules] = useState<RuleCatalog | null>(null);
  const [fixtures, setFixtures] = useState<FixtureSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [patterns, setPatterns] = useState<string[]>(settings.preservePatterns);
  const [selectedFixtureId, setSelectedFixtureId] = useState("");
  const [sampleReload, setSampleReload] = useState(0);
  const [sampleBusy, setSampleBusy] = useState(false);
  const [sampleError, setSampleError] = useState<string | null>(null);
  const [testText, setTestText] = useState("");
  const [inspectBusy, setInspectBusy] = useState(false);
  const [inspectError, setInspectError] = useState<string | null>(null);
  const [inspection, setInspection] = useState<ProtectionInspection | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const inspectAbort = useRef<AbortController | null>(null);
  const sampleAbort = useRef<AbortController | null>(null);

  useEffect(() => {
    setPatterns(settings.preservePatterns);
  }, [settings.preservePatterns]);

  useEffect(() => {
    const controller = new AbortController();
    setLoadError(null);
    Promise.all([getRules(controller.signal), getFixtures(controller.signal)])
      .then(([catalog, fixtureList]) => {
        setRules(catalog);
        setFixtures(fixtureList);
        setSelectedFixtureId((current) => {
          if (current && fixtureList.some((fixture) => fixture.id === current)) {
            return current;
          }
          return (
            fixtureList.find((fixture) => fixture.id === "rust-build-failure")?.id ??
            fixtureList[0]?.id ??
            ""
          );
        });
      })
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setLoadError(errorMessage(caught));
        }
      });
    return () => controller.abort();
  }, [attempt]);

  useEffect(() => {
    if (!selectedFixtureId) {
      setSampleBusy(false);
      return;
    }
    sampleAbort.current?.abort();
    const controller = new AbortController();
    sampleAbort.current = controller;
    setSampleBusy(true);
    setSampleError(null);
    setInspection(null);
    setInspectError(null);
    getFixture(selectedFixtureId, controller.signal)
      .then((detail) => {
        setTestText(detail.content);
        setInspection(null);
        setInspectError(null);
      })
      .catch((caught: unknown) => {
        if (!isAbortError(caught)) {
          setSampleError(errorMessage(caught));
        }
      })
      .finally(() => {
        if (sampleAbort.current === controller) {
          sampleAbort.current = null;
          setSampleBusy(false);
        }
      });
    return () => controller.abort();
  }, [selectedFixtureId, sampleReload]);

  useEffect(() => {
    return () => {
      inspectAbort.current?.abort();
      sampleAbort.current?.abort();
    };
  }, []);

  const activePatterns = settings.preservePatterns;
  const dirty = JSON.stringify(patterns) !== JSON.stringify(activePatterns);
  const inputBytes = useMemo(() => utf8Bytes(testText), [testText]);
  const inputLines = useMemo(() => countLines(testText), [testText]);
  const overLimit = inputBytes > settings.maxInputBytes;
  const ruleList = rules?.rules ?? [];
  const canInspect = testText.length > 0 && !overLimit && !inspectBusy && !sampleBusy;

  const runInspection = useCallback(async () => {
    if (!canInspect) {
      return;
    }
    inspectAbort.current?.abort();
    const controller = new AbortController();
    inspectAbort.current = controller;
    setInspectBusy(true);
    setInspectError(null);
    try {
      const result = await inspectInput(
        {
          text: testText,
          preserve_patterns: patterns,
          max_input_bytes: settings.maxInputBytes,
        },
        controller.signal,
      );
      setInspection(result);
    } catch (caught: unknown) {
      if (!isAbortError(caught)) {
        setInspectError(errorMessage(caught));
      }
    } finally {
      if (inspectAbort.current === controller) {
        inspectAbort.current = null;
        setInspectBusy(false);
      }
    }
  }, [canInspect, patterns, settings.maxInputBytes, testText]);

  async function loadTestFile(files: FileList | null) {
    const file = files?.[0];
    if (!file) return;
    setSampleError(null);
    if (file.size > settings.maxInputBytes) {
      setSampleError(
        `File is ${formatBytes(file.size)}; the current limit is ${formatBytes(
          settings.maxInputBytes,
        )}.`,
      );
      return;
    }
    try {
      const decoded = await readUtf8File(file);
      if (utf8Bytes(decoded) > settings.maxInputBytes) {
        setSampleError(`Decoded input exceeds the ${formatBytes(settings.maxInputBytes)} limit.`);
        return;
      }
      setTestText(decoded);
      setSelectedFixtureId("");
      setInspection(null);
      setInspectError(null);
    } catch {
      setSampleError("The selected file is not valid UTF-8 text or could not be read.");
    }
  }

  function applyPatterns() {
    saveSettings({ ...settings, preservePatterns: patterns });
  }

  function discardPatterns() {
    setPatterns(activePatterns);
  }

  return (
    <>
      <PageHeader
        index="03 / Rules"
        title="Rules"
        description="Inspect the heuristic preservation map, add literal patterns for your own evidence, and see exactly which lines compression is not allowed to remove."
        actions={
          <Link
            to="/workspace"
            className="inline-flex min-h-10 items-center gap-2 border border-line-strong bg-surface px-3.5 py-2 text-[13px] font-medium text-ink transition-colors hover:border-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust"
          >
            Test in Workspace
            <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
          </Link>
        }
        meta={
          <>
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Built-in rules <span className="text-ink">{ruleList.length || "—"}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Custom patterns <span className="text-ink">{activePatterns.length}</span>
            </span>
            <span className="hidden h-3 w-px bg-line sm:block" />
            <span className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Matching{" "}
              <span className="text-ink">{dirty ? "draft rules active in inspector" : "applied"}</span>
            </span>
            <span className="ml-auto font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              heuristic protection
            </span>
          </>
        }
      />

      {loadError ? (
        <div className="mb-4">
          <ErrorNotice
            title="Rules could not be loaded"
            message={loadError}
            onRetry={() => setAttempt((value) => value + 1)}
          />
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
        <PatternEditor
          patterns={patterns}
          setPatterns={(next) => {
            setPatterns(next);
            setInspection(null);
          }}
          activePatterns={activePatterns}
          onApply={applyPatterns}
          onDiscard={discardPatterns}
          persisted={persisted}
        />

        <aside className="border border-line bg-surface p-4 sm:p-5">
          <div className="flex items-center gap-2">
            <ShieldCheck aria-hidden="true" className="h-4 w-4 text-moss" />
            <h2 className="text-[15px] font-semibold">How protection behaves</h2>
          </div>
          <div className="mt-4 space-y-4 border-t border-line pt-4">
            {[
              [
                "Protection wins",
                "Validation failing or output not being smaller returns the original unchanged.",
              ],
              [
                "Adjacent context",
                "Lines immediately surrounding protected evidence are retained so logs keep their diagnostic frame.",
              ],
              [
                "Deterministic",
                "The same input, options, and rule list produce the same output. No model is involved.",
              ],
              [
                "Heuristic, not proof",
                "Rules can over-protect or miss unusual formats. Review the diff and add specific patterns when needed.",
              ],
            ].map(([title, body]) => (
              <div key={title}>
                <p className="text-[12px] font-medium text-ink">{title}</p>
                <p className="mt-0.5 text-[11.5px] leading-5 text-muted">{body}</p>
              </div>
            ))}
          </div>
          <div className="mt-4 border-t border-line pt-3">
            <p className="font-mono text-[9px] uppercase tracking-[0.1em] text-muted">
              Applied patterns are stored locally only
            </p>
            <Link
              to="/settings"
              className="mt-1 inline-block text-[11.5px] text-rust underline decoration-rust/30 hover:decoration-rust"
            >
              Review local storage behavior
            </Link>
          </div>
        </aside>
      </div>

      <section aria-labelledby="inspector-heading" className="mt-6 border border-line bg-surface">
        <div className="flex flex-wrap items-start justify-between gap-3 border-b border-line p-4 sm:p-5">
          <div>
            <h2 id="inspector-heading" className="text-[15px] font-semibold">
              Protection inspector
            </h2>
            <p className="mt-1.5 max-w-2xl text-[12.5px] leading-5 text-muted">
              Run the real protection map without compressing anything. The inspector uses the
              patterns currently in the editor, including unapplied drafts.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={dirty ? "warning" : "neutral"}>
              {dirty ? "draft patterns" : "applied patterns"}
            </Badge>
            <Badge tone="success">no transformation</Badge>
          </div>
        </div>

        <div className="grid lg:grid-cols-2">
          <div className="border-b border-line p-4 sm:p-5 lg:border-b-0 lg:border-r">
            <div className="flex flex-wrap items-end gap-2">
              <Select
                label="Load a fixture"
                value={selectedFixtureId}
                onChange={(event) => {
                  setSelectedFixtureId(event.target.value);
                  setInspection(null);
                  setInspectError(null);
                }}
                disabled={inspectBusy || fixtures.length === 0}
                containerClassName="min-w-[190px] flex-1"
                className="h-9 text-[12px]"
              >
                <option value="">Choose fixture…</option>
                {fixtures.map((fixture) => (
                  <option key={fixture.id} value={fixture.id}>
                    {fixture.name}
                  </option>
                ))}
              </Select>
              <Button
                variant="ghost"
                size="sm"
                disabled={!selectedFixtureId || sampleBusy}
                onClick={() => {
                  setInspection(null);
                  setSampleReload((value) => value + 1);
                }}
              >
                <RotateCcw aria-hidden="true" className="h-3.5 w-3.5" />
                Reload
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                accept=".log,.txt,.json,.jsonl,.ndjson,.out,.trace,.md"
                onChange={(event) => {
                  const files = event.currentTarget.files;
                  void loadTestFile(files);
                  event.currentTarget.value = "";
                }}
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => fileInputRef.current?.click()}
                disabled={inspectBusy}
              >
                Open file
              </Button>
            </div>

            <div className="relative mt-3">
              <textarea
                value={testText}
                onChange={(event) => {
                  setTestText(event.target.value);
                  if (selectedFixtureId) setSelectedFixtureId("");
                  setInspection(null);
                  setInspectError(null);
                  setSampleError(null);
                }}
                aria-label="Input to inspect for protected lines"
                spellCheck={false}
                placeholder="Paste a real log line, test output, stack trace, or structured log…"
                className="field-input field-textarea min-h-[19rem] w-full"
              />
              {sampleBusy ? (
                <div className="absolute inset-0 grid place-items-center bg-surface/75">
                  <span className="font-mono text-[10px] uppercase tracking-[0.1em] text-muted">
                    Loading fixture…
                  </span>
                </div>
              ) : null}
            </div>
            <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
              <p className="font-mono text-[9.5px] text-muted">
                {formatNumber(inputBytes)} bytes · {pluralize(inputLines, "line")} ·{" "}
                {formatBytes(settings.maxInputBytes)} limit
              </p>
              <span className="font-mono text-[9px] uppercase tracking-[0.08em] text-muted">
                inspect only
              </span>
            </div>
            {overLimit ? (
              <p className="mt-2 border border-danger/25 bg-danger-soft/50 px-3 py-2 text-[12px] text-danger">
                Input exceeds the current limit. Reduce it or raise the limit in Settings.
              </p>
            ) : null}
            {sampleError ? (
              <p role="alert" className="mt-2 text-[11.5px] leading-5 text-danger">
                {sampleError}
              </p>
            ) : null}
            {inspectError ? (
              <div className="mt-2">
                <ErrorNotice
                  title="Inspection failed"
                  message={inspectError}
                  onRetry={canInspect ? () => void runInspection() : undefined}
                />
              </div>
            ) : null}

            <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-line pt-3">
              <Button
                variant="primary"
                size="sm"
                loading={inspectBusy}
                disabled={!canInspect}
                onClick={() => void runInspection()}
              >
                {inspectBusy ? "Inspecting…" : "Inspect protection"}
                {!inspectBusy ? <ScanSearch aria-hidden="true" className="h-3.5 w-3.5" /> : null}
              </Button>
              <span className="text-[11px] leading-5 text-muted">
                Protected lines are never candidates for omission.
              </span>
            </div>
          </div>

          <div className="p-4 sm:p-5">
            {sampleBusy && !inspection ? (
              <div className="space-y-3" aria-label="Loading inspection source">
                <div className="skeleton h-20 w-full" />
                <div className="skeleton h-36 w-full" />
              </div>
            ) : inspection ? (
              <InspectionResult inspection={inspection} rules={ruleList} />
            ) : (
              <EmptyState
                icon={ScanSearch}
                title="See the protection map."
                description="Inspect a fixture or paste your own input. Results show every protected line, each matching reason, and coverage for the current literal patterns."
              >
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!canInspect}
                  onClick={() => void runInspection()}
                >
                  Inspect current input
                  <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
                </Button>
              </EmptyState>
            )}
          </div>
        </div>
      </section>

      <div className="mt-6 grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(300px,0.9fr)]">
        <section aria-labelledby="built-in-heading" className="border border-line bg-surface">
          <div className="flex flex-wrap items-start justify-between gap-3 border-b border-line p-4 sm:p-5">
            <div>
              <h2 id="built-in-heading" className="text-[15px] font-semibold">
                Built-in protection rules
              </h2>
              <p className="mt-1.5 text-[12.5px] leading-5 text-muted">
                Stable public metadata from the engine. The patterns themselves stay in the local
                Rust core.
              </p>
            </div>
            <Badge tone="neutral">{ruleList.length} rules</Badge>
          </div>
          {rules ? (
            <ul className="divide-y divide-line">
              {rules.rules.map((rule) => (
                <li key={rule.id} className="grid gap-1.5 px-4 py-3 sm:grid-cols-[170px_1fr] sm:gap-4 sm:px-5">
                  <div>
                    <code className="font-mono text-[9.5px] uppercase tracking-[0.06em] text-rust">
                      {rule.id}
                    </code>
                    <p className="mt-0.5 text-[11px] leading-4 text-muted">{rule.label}</p>
                  </div>
                  <p className="text-[11.5px] leading-5 text-ink-soft">
                    {RULE_NOTES[rule.id] ?? "Heuristic preserve rule."}
                  </p>
                </li>
              ))}
            </ul>
          ) : (
            <div className="space-y-2 p-5">
              <div className="skeleton h-8 w-full" />
              <div className="skeleton h-8 w-5/6" />
            </div>
          )}
          {rules ? (
            <p className="border-t border-line px-4 py-3 text-[10.5px] leading-5 text-muted sm:px-5">
              {rules.note}
            </p>
          ) : null}
        </section>

        <section aria-labelledby="boundaries-heading" className="border border-line bg-surface p-4 sm:p-5">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 id="boundaries-heading" className="text-[15px] font-semibold">
                Strategy boundaries
              </h2>
              <p className="mt-1.5 text-[12.5px] leading-5 text-muted">
                Protection is separate from compression. Each strategy only removes eligible spans.
              </p>
            </div>
            <ListChecks aria-hidden="true" className="h-4 w-4 shrink-0 text-muted" />
          </div>
          <ol className="mt-4 divide-y divide-line border-y border-line">
            {STRATEGY_BOUNDARIES.map((strategy, index) => (
              <li key={strategy.label} className="py-3.5">
                <div className="flex items-baseline gap-2">
                  <span className="font-mono text-[9px] text-rust">0{index + 1}</span>
                  <h3 className="text-[13px] font-medium">{strategy.label}</h3>
                </div>
                <p className="mt-1 text-[11.5px] leading-5 text-ink-soft">{strategy.removes}</p>
                <p className="mt-1 text-[11px] leading-5 text-muted">{strategy.note}</p>
              </li>
            ))}
          </ol>
          <div className="mt-4 flex items-start gap-2.5 border border-amber/25 bg-amber-soft/35 p-3">
            <TriangleAlert aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-amber-dark" />
            <p className="text-[11px] leading-5 text-amber-dark">
              Not semantic equivalence. Heuristic protections can miss unusual evidence. Always
              review the diff before using compressed context.
            </p>
          </div>
        </section>
      </div>
    </>
  );
}
