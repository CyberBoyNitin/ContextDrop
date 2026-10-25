import type {
  CostEstimate,
  InputFormat,
  InputKind,
  Pricing,
  Strategy,
  TokenizerKind,
} from "./types";

export function cn(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}

const integerFormatter = new Intl.NumberFormat("en-US");

export function formatNumber(value: number): string {
  return integerFormatter.format(value);
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${formatNumber(bytes)} B`;
}

export function formatPercent(value: number, digits = 1): string {
  if (!Number.isFinite(value)) {
    return "—";
  }
  return `${value.toFixed(digits)}%`;
}

export function formatSignedPercent(value: number, digits = 1): string {
  if (!Number.isFinite(value)) {
    return "—";
  }
  return `${value >= 0 ? "−" : "+"}${Math.abs(value).toFixed(digits)}%`;
}

export function formatMs(value: number): string {
  if (!Number.isFinite(value)) {
    return "—";
  }
  if (value < 1) {
    return `${value.toFixed(2)} ms`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(2)} s`;
  }
  return `${value.toFixed(1)} ms`;
}

export function formatCost(cost: CostEstimate, value: number): string {
  return `${cost.currency} ${value.toFixed(6)}`;
}

export function utf8Bytes(value: string): number {
  return new TextEncoder().encode(value).length;
}

export function countLines(value: string): number {
  if (!value) {
    return 0;
  }
  // Match the Rust engine: a trailing line ending terminates the final
  // content line rather than creating an extra empty line. A bare carriage
  // return stays in the line content.
  const normalized = value.replace(/\r\n/g, "\n");
  const lines = normalized.split("\n");
  if (normalized.endsWith("\n")) {
    lines.pop();
  }
  return lines.length;
}

export function strategyLabel(strategy: Strategy): string {
  if (strategy === "conservative") return "Conservative";
  if (strategy === "balanced") return "Balanced";
  return "Aggressive";
}

export function strategyShortLabel(strategy: Strategy): string {
  if (strategy === "conservative") return "Safe";
  if (strategy === "balanced") return "Balanced";
  return "Deep";
}

export function strategyDescription(strategy: Strategy): string {
  if (strategy === "conservative") {
    return "Collapses consecutive repeated lines only. The narrowest deterministic change.";
  }
  if (strategy === "balanced") {
    return "Also removes passing-test noise, progress updates, and exact duplicate JSON records.";
  }
  return "Adds global duplicate removal and normalized structured-log grouping. Review every diff.";
}

export function inputFormatLabel(format: InputFormat): string {
  switch (format) {
    case "auto":
      return "Auto-detect";
    case "plain-text":
      return "Plain text";
    case "terminal":
      return "Terminal";
    case "build-log":
      return "Build log";
    case "test-output":
      return "Test output";
    case "json":
      return "JSON";
    case "structured-log":
      return "Structured log";
  }
}

export function inputKindLabel(kind: InputKind): string {
  return inputFormatLabel(kind);
}

export function tokenizerLabel(tokenizer: TokenizerKind): string {
  switch (tokenizer) {
    case "o200k_base":
      return "o200k_base · exact";
    case "cl100k_base":
      return "cl100k_base · exact";
    case "estimate":
      return "Heuristic estimate";
  }
}

export function tokenizerShortLabel(tokenizer: TokenizerKind): string {
  switch (tokenizer) {
    case "o200k_base":
      return "o200k";
    case "cl100k_base":
      return "cl100k";
    case "estimate":
      return "estimate";
  }
}

export function pricingFromSettings(
  enabled: boolean,
  rawPrice: string,
  currency: string,
): Pricing | undefined {
  if (!enabled) {
    return undefined;
  }
  const price = Number(rawPrice);
  const cleanCurrency = currency.trim().toUpperCase();
  if (!Number.isFinite(price) || price < 0 || price > 1_000_000) {
    return undefined;
  }
  if (!/^[A-Z0-9]{3,8}$/.test(cleanCurrency)) {
    return undefined;
  }
  return {
    input_per_million: price,
    currency: cleanCurrency,
  };
}

export function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${formatNumber(count)} ${count === 1 ? singular : plural}`;
}

export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export function downloadText(filename: string, text: string, mime = "text/plain;charset=utf-8"): void {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function copyText(text: string): Promise<boolean> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // Fall through to the legacy path when clipboard permissions are denied.
    }
  }
  try {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    const copied = document.execCommand("copy");
    textarea.remove();
    return copied;
  } catch {
    return false;
  }
}

export function safeDownloadName(value: string): string {
  const cleaned = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return cleaned || "contextdrop-result";
}

export function fileExtension(inputFormat: InputFormat): string {
  if (inputFormat === "json") return "json";
  if (inputFormat === "test-output") return "test.log";
  if (inputFormat === "build-log") return "build.log";
  if (inputFormat === "structured-log") return "jsonl";
  return "txt";
}

export function warningsForDisplay(warnings: string[]): string[] {
  return [...new Set(warnings.filter(Boolean))];
}

export async function readUtf8File(file: File): Promise<string> {
  const bytes = await file.arrayBuffer();
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

export function fileNameWithoutExtension(name: string): string {
  const base = name.split(/[\\/]/).pop() ?? name;
  return base.replace(/\.[^.]+$/, "").slice(0, 120) || "Pasted input";
}
