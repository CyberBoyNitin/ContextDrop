export type Strategy = "conservative" | "balanced" | "aggressive";

export type InputFormat =
  | "auto"
  | "plain-text"
  | "terminal"
  | "build-log"
  | "test-output"
  | "json"
  | "structured-log";

export type InputKind =
  | "plain-text"
  | "terminal"
  | "build-log"
  | "test-output"
  | "json"
  | "structured-log";

export type TokenizerKind = "o200k_base" | "cl100k_base" | "estimate";

export type TokenMeasurementKind = "exact" | "estimated";

export interface Pricing {
  input_per_million: number;
  currency: string;
}

export interface Classification {
  kind: InputKind;
  automatic: boolean;
  confidence: number;
}

export interface TokenMeasurement {
  kind: TokenMeasurementKind;
  name: string;
  label: string;
  method: string;
  note: string;
}

export interface CostEstimate {
  currency: string;
  input_price_per_million: number;
  original_input_cost: number;
  compressed_input_cost: number;
  input_cost_saved: number;
  note: string;
}

export interface Metrics {
  original_bytes: number;
  output_bytes: number;
  bytes_saved: number;
  byte_reduction_percent: number;
  original_chars: number;
  output_chars: number;
  chars_saved: number;
  original_tokens: number;
  output_tokens: number;
  tokens_saved: number;
  token_reduction_percent: number;
  token_measurement: TokenMeasurement;
  processing_time_ms: number;
  cost: CostEstimate | null;
}

export interface Omissions {
  repeated_line_runs: number;
  repeated_lines_omitted: number;
  passing_test_lines_omitted: number;
  progress_lines_omitted: number;
  normalized_log_lines_omitted: number;
  json_duplicate_items_omitted: number;
  markers_inserted: number;
  total_input_lines: number;
  total_output_lines: number;
}

export interface PreservationCheck {
  id: string;
  label: string;
  passed: boolean;
  detail: string;
}

export interface PreservationReport {
  passed: boolean;
  protected_occurrences: number;
  retained_occurrences: number;
  checks: PreservationCheck[];
  note: string;
}

export interface CompressionResult {
  output: string;
  strategy: Strategy;
  classification: Classification;
  metrics: Metrics;
  omissions: Omissions;
  preservation: PreservationReport;
  warnings: string[];
  fallback_used: boolean;
  fallback_reason: string | null;
  transformation_applied: boolean;
}

export interface CompressionRequest {
  text: string;
  strategy: Strategy;
  input_format: InputFormat;
  tokenizer: TokenizerKind;
  preserve_patterns: string[];
  pricing?: Pricing;
  collapse_repetitions: boolean;
  collapse_passing_tests: boolean;
  collapse_progress_updates: boolean;
  structural_json: boolean;
  max_input_bytes: number;
}

export type BenchmarkSource = "built-in" | "user";

export interface FixtureSummary {
  id: string;
  name: string;
  description: string;
  category: string;
  format: InputFormat;
  original_bytes: number;
  assertion_count: number;
  notes: string | null;
}

export interface FixtureDetail {
  fixture: FixtureSummary;
  content: string;
}

export interface BenchmarkCaseInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  format: InputFormat;
  source: BenchmarkSource;
  original_bytes: number;
  must_preserve: string[];
  notes: string | null;
}

export interface CriticalCheck {
  label: string;
  pattern: string;
  found_in_original: boolean;
  passed: boolean;
  detail: string;
}

export interface BenchmarkStrategyResult {
  strategy: Strategy;
  compression: CompressionResult;
  critical_checks: CriticalCheck[];
}

export interface BenchmarkRun {
  case: BenchmarkCaseInfo;
  results: BenchmarkStrategyResult[];
}

export interface BenchmarkRequest {
  fixture_id?: string;
  text?: string;
  name?: string;
  input_format: InputFormat;
  tokenizer: TokenizerKind;
  preserve_patterns: string[];
  pricing?: Pricing;
  collapse_repetitions: boolean;
  collapse_passing_tests: boolean;
  collapse_progress_updates: boolean;
  structural_json: boolean;
  max_input_bytes: number;
}

export interface RuleInfo {
  id: string;
  label: string;
}

export interface RuleCatalog {
  rules: RuleInfo[];
  user_patterns: string;
  note: string;
}

export interface ProtectedLine {
  line_number: number;
  text: string;
  reasons: string[];
}

export interface ProtectionInspection {
  total_lines: number;
  protected_lines: number;
  reason_counts: Record<string, number>;
  matches: ProtectedLine[];
  truncated: boolean;
  note: string;
}

export interface InspectRequest {
  text: string;
  preserve_patterns: string[];
  max_input_bytes: number;
}

export interface HealthResponse {
  status: string;
  api_version: number;
  engine_version: string;
  max_input_bytes: number;
}

export interface ApiErrorEnvelope {
  error?: {
    code?: string;
    message?: string;
  };
}
