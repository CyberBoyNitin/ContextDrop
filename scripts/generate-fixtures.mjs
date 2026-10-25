#!/usr/bin/env node
// Deterministic fixture generator for ContextDrop regression and benchmark data.
// All generated content is synthetic. No user or production data is included.
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("../fixtures", import.meta.url));
mkdirSync(root, { recursive: true });

const writeText = (name, text) => {
  const content = text.endsWith("\n") ? text : `${text}\n`;
  writeFileSync(join(root, name), content, "utf8");
  return content;
};

const fixtures = [];

function addFixture(spec, content) {
  writeText(spec.file, content);
  fixtures.push(spec);
}

// 1. Rust build failure: unique compiler work plus repeated lock waits.
{
  const lines = ["$ cargo build --release"];
  for (let i = 1; i <= 72; i += 1) {
    lines.push(`   Compiling fixture-dependency-${String(i).padStart(2, "0")} v0.1.${i}`);
  }
  for (let i = 0; i < 38; i += 1) {
    lines.push("    Blocking waiting for file lock on build directory");
  }
  lines.push(
    "warning: unused variable: `retries`",
    "  --> crates/contextdrop-core/src/config.rs:42:9",
    "   |",
    "42 |     let retries = 3;",
    "   |         ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_retries`",
    "   |",
    "   = note: `#[warn(unused_variables)]` on by default",
    "",
    "error[E0308]: mismatched types",
    " --> crates/contextdrop-cli/src/main.rs:14:37",
    "   |",
    "14 |     let message: String = load_name();",
    "   |                  ------   ^^^^^^^^^^^ expected `String`, found `&str`",
    "   |                  |",
    "   |                  expected due to this",
    "",
    "error: aborting due to 1 previous error",
    "error: could not compile `contextdrop-cli` (bin \"contextdrop\") due to 1 previous error",
    "exit code: 101",
  );
  addFixture(
    {
      id: "rust-build-failure",
      name: "Rust compiler failure",
      description:
        "Cargo build with repeated file-lock waits, compiler diagnostics, source excerpt, and exit code.",
      file: "rust-build-failure.log",
      category: "Build failure",
      format: "build-log",
      must_preserve: [
        "error[E0308]: mismatched types",
        "crates/contextdrop-cli/src/main.rs:14:37",
        "expected `String`, found `&str`",
        "error: could not compile `contextdrop-cli`",
        "exit code: 101",
        "warning: unused variable: `retries`",
      ],
      notes:
        "Synthetic Cargo output. Repeated lock waits are compressible; all diagnostics and source-location evidence are protected.",
    },
    lines.join("\n"),
  );
}

// 2. TypeScript/Vite build failure with progress noise.
{
  const lines = [
    "$ npm run build",
    "> studio-web@2.4.0 build",
    "> tsc -p tsconfig.build.json --noEmit && vite build",
    "vite v6.2.0 building for production...",
    "transforming...",
  ];
  for (let i = 0; i < 160; i += 1) {
    const percent = Math.min(99, Math.floor(((i + 1) / 160) * 100));
    const filled = Math.round(percent / 5);
    const bar = "#".repeat(filled).padEnd(20, "-");
    lines.push(`[${bar}] ${percent}% transforming modules`);
  }
  for (let i = 0; i < 36; i += 1) {
    lines.push("Waiting for TypeScript to report diagnostics...");
  }
  lines.push(
    "src/server/session.ts(42,17): error TS2322: Type 'string' is not assignable to type 'SessionUser'.",
    "src/routes/auth.ts(19,3): error TS2345: Argument of type 'undefined' is not assignable to parameter of type 'string'.",
    "src/utils/format.ts(6,11): warning TS6133: 'unused' is declared but its value is never read.",
    "",
    "Build failed with 2 errors.",
    "exit code: 1",
  );
  addFixture(
    {
      id: "typescript-build-failure",
      name: "TypeScript build failure",
      description:
        "Vite/TypeScript build with 160 progress updates and two type errors with source locations.",
      file: "typescript-build-failure.log",
      category: "Build failure",
      format: "build-log",
      must_preserve: [
        "src/server/session.ts(42,17): error TS2322",
        "src/routes/auth.ts(19,3): error TS2345",
        "warning TS6133",
        "Build failed with 2 errors",
        "exit code: 1",
      ],
      notes:
        "Progress updates are safe to omit under balanced/aggressive rules; all errors, warnings, and the final failure summary are protected.",
    },
    lines.join("\n"),
  );
}

// 3. Jest test failure with many passing suites.
{
  const lines = [];
  for (let suite = 1; suite <= 9; suite += 1) {
    lines.push(`PASS tests/unit/module-${suite}.test.ts`);
    for (let test = 1; test <= 16; test += 1) {
      lines.push(`  ✓ module ${suite} handles case ${String(test).padStart(2, "0")} (${(suite + test) % 9 + 1} ms)`);
    }
    lines.push("");
  }
  lines.push(
    "FAIL tests/unit/cache.test.ts",
    "  ● CacheService › expires stale entries",
    "",
    "    expect(received).toBe(expected)",
    "",
    "    Expected: true",
    "    Received: false",
    "",
    "      at Object.<anonymous> (tests/unit/cache.test.ts:28:17)",
    "      at async Promise.all (index 3)",
    "",
    "Test Suites: 1 failed, 9 passed, 10 total",
    "Tests:       1 failed, 143 passed, 144 total",
    "Snapshots:   0 total",
    "Time:        2.118 s",
    "Ran all test suites.",
  );
  addFixture(
    {
      id: "jest-test-failure",
      name: "Jest suite with one failure",
      description:
        "A 144-test Jest run with 143 passing tests, a failed assertion, code frame, and summary.",
      file: "jest-test-failure.log",
      category: "Test failure",
      format: "test-output",
      must_preserve: [
        "CacheService › expires stale entries",
        "Expected: true",
        "Received: false",
        "tests/unit/cache.test.ts:28:17",
        "1 failed, 143 passed, 144 total",
      ],
      notes:
        "Passing entries are compressible; the failing test, assertion values, source location, and totals are protected.",
    },
    lines.join("\n"),
  );
}

// 4. Pytest failure with a traceback.
{
  const lines = [
    "$ pytest -q tests",
    "platform win32 -- Python 3.13.2, pytest-8.3.5",
    "rootdir: C:\\project",
  ];
  for (let i = 1; i <= 120; i += 1) {
    lines.push(
      `tests/test_parser.py::test_parse_case_${String(i).padStart(3, "0")} PASSED [${String(Math.floor((i / 120) * 100)).padStart(3)}%]`,
    );
  }
  lines.push(
    "",
    "=================================== FAILURES ===================================",
    "________________________ test_rejects_empty_input _____________________________",
    "",
    "    def test_rejects_empty_input():",
    '>       result = parse("")',
    "E       AssertionError: expected ValueError, got None",
    "",
    "tests/test_parser.py:88: AssertionError",
    "=========================== short test summary info ============================",
    "FAILED tests/test_parser.py::test_rejects_empty_input - AssertionError: expected ValueError, got None",
    "1 failed, 120 passed, 4 warnings in 2.31s",
  );
  addFixture(
    {
      id: "pytest-traceback",
      name: "Pytest failure and traceback",
      description:
        "Pytest verbose output with 120 passing tests, a failing assertion, code frame, and traceback location.",
      file: "pytest-traceback.log",
      category: "Test failure",
      format: "test-output",
      must_preserve: [
        "FAILED tests/test_parser.py::test_rejects_empty_input",
        "AssertionError: expected ValueError, got None",
        "tests/test_parser.py:88: AssertionError",
        'result = parse("")',
        "1 failed, 120 passed, 4 warnings",
      ],
      notes:
        "The passing-test block is compressed; the failing test, assertion detail, traceback location, and summary remain.",
    },
    lines.join("\n"),
  );
}

// 5. npm dependency installation with progress, warnings, and audit findings.
{
  const lines = ["$ npm ci --verbose"];
  for (let i = 0; i < 240; i += 1) {
    const percent = Math.min(99, Math.floor(((i + 1) / 240) * 100));
    const filled = Math.round(percent / 5);
    const bar = "#".repeat(filled).padEnd(20, "-");
    lines.push(`[${bar}] ${percent}% downloading package payload`);
  }
  lines.push(
    "npm WARN deprecated inflight@1.0.6: This version is not supported. Please update.",
    "npm WARN EBADENGINE Unsupported engine { package: 'vite@7.2.0', required: { node: '>=20' } }",
    "npm http fetch GET 200 https://registry.npmjs.org/rollup 214ms (cache revalidated)",
    "added 128 packages, and audited 129 packages in 6s",
    "found 3 vulnerabilities (1 moderate, 2 high)",
    "run `npm audit fix` to fix them, or `npm audit` for details",
  );
  addFixture(
    {
      id: "npm-install-progress",
      name: "npm install progress",
      description:
        "Dependency installation with 240 progress bars, deprecation warnings, and vulnerability findings.",
      file: "npm-install-progress.log",
      category: "Dependencies",
      format: "build-log",
      must_preserve: [
        "npm WARN deprecated inflight@1.0.6",
        "npm WARN EBADENGINE Unsupported engine",
        "added 128 packages",
        "found 3 vulnerabilities",
      ],
      notes:
        "Progress bars are collapsed while warnings and security findings are protected. This intentionally retains most warning bulk.",
    },
    lines.join("\n"),
  );
}

// 6. Structured HTTP access log with many repeated health checks.
{
  const lines = [];
  const base = Date.UTC(2026, 9, 20, 14, 5, 0);
  const stamp = (offsetMs) => new Date(base + offsetMs).toISOString();
  const requestId = (n) =>
    `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
  for (let i = 0; i < 180; i += 1) {
    lines.push(
      `${stamp(i * 700)} INFO http.request method=GET path=/health status=200 duration_ms=${(i % 7) + 1} request_id=${requestId(i)}`,
    );
  }
  lines.push(
    `${stamp(180 * 700)} ERROR http.request method=POST path=/api/orders status=500 duration_ms=37 request_id=${requestId(999)}`,
    "upstream payment service unavailable",
    "    at handler (src/routes/orders.ts:88:11)",
  );
  for (let i = 181; i < 320; i += 1) {
    lines.push(
      `${stamp(i * 700)} INFO http.request method=GET path=/health status=200 duration_ms=${(i % 7) + 1} request_id=${requestId(i)}`,
    );
  }
  addFixture(
    {
      id: "structured-http-log",
      name: "Structured HTTP access log",
      description:
        "320 structured request logs around one 500 response and handler stack location.",
      file: "structured-http.log",
      category: "Structured logs",
      format: "structured-log",
      must_preserve: [
        "path=/api/orders",
        "status=500",
        "upstream payment service unavailable",
        "src/routes/orders.ts:88:11",
      ],
      notes:
        "Balanced mode has few exact duplicates because IDs differ. Aggressive normalized grouping is expected to compress repeated health-check templates.",
    },
    lines.join("\n"),
  );
}

// 7. Large JSON record array with duplicate healthy records and protected errors.
{
  const healthy = {
    id: 7,
    level: "info",
    message: "order accepted",
    status: 200,
    latency_ms: 12,
    trace_id: "trace-ok-0001",
  };
  const errors = [
    {
      id: 901,
      level: "error",
      message: "payment capture failed",
      status: 502,
      error_code: "PAYMENT_DECLINED",
      trace_id: "trace-9f3a2b",
    },
    {
      id: 902,
      level: "error",
      message: "inventory reservation expired",
      status: 409,
      error_code: "INVENTORY_EXPIRED",
      trace_id: "trace-44c102",
    },
    {
      id: 903,
      level: "warn",
      message: "upstream settlement timeout",
      status: 504,
      error_code: "SETTLEMENT_TIMEOUT",
      trace_id: "trace-7ab983",
    },
  ];
  const records = [];
  for (let i = 0; i < 20; i += 1) records.push(healthy);
  records.push(...errors);
  for (let i = 0; i < 240; i += 1) records.push(healthy);
  const object = {
    schema: "contextdrop.benchmark/v1",
    generated: "2026-10-20T14:05:00Z",
    records,
  };
  addFixture(
    {
      id: "json-duplicate-records",
      name: "Large JSON duplicate array",
      description:
        "A 263-record JSON array with 260 exact duplicate healthy records and three protected error records.",
      file: "large-json-array.json",
      category: "Structured JSON",
      format: "json",
      must_preserve: [
        "payment capture failed",
        "PAYMENT_DECLINED",
        "trace-9f3a2b",
        "inventory reservation expired",
        "upstream settlement timeout",
      ],
      notes:
        "Balanced structural deduplication is expected to reduce this input substantially while retaining every protected error record.",
    },
    JSON.stringify(object, null, 2),
  );
}

// 8. Security scan where protected findings dominate.
{
  const lines = ["$ security-scan --all"];
  for (let i = 0; i < 90; i += 1) {
    lines.push(`Scanning dependency graph... ${String(i + 1).padStart(2, "0")}ms`);
  }
  lines.push(
    "[CRITICAL] CVE-2026-21894 vulnerable dependency openssl 3.2.0 in Cargo.lock:412",
    "[CRITICAL] hard-coded credential detected in src/auth/session.rs:77",
    "[HIGH] SQL injection risk in src/db/query.ts:203",
    "[HIGH] authentication failed for admin after 5 attempts",
    "[MEDIUM] insecure random source in src/crypto/nonce.py:14",
  );
  for (let i = 0; i < 60; i += 1) {
    lines.push("Waiting for scanner workers to finish...");
  }
  lines.push(
    "[CRITICAL] CVE-2026-21894 remains unresolved after remediation attempt",
    "Security scan complete: 3 critical, 5 high, 4 medium",
    "exit code: 2",
  );
  addFixture(
    {
      id: "security-scan",
      name: "Security scan findings",
      description:
        "A scanner report with critical vulnerabilities, credential exposure, injection findings, and repeated worker noise.",
      file: "security-scan.log",
      category: "Security evidence",
      format: "plain-text",
      must_preserve: [
        "CVE-2026-21894",
        "src/auth/session.rs:77",
        "hard-coded credential",
        "authentication failed",
        "Security scan complete: 3 critical, 5 high, 4 medium",
      ],
      notes:
        "Most semantic content is protected evidence, intentionally limiting compression. The repeated scanner waits are compressible.",
    },
    lines.join("\n"),
  );
}

// 9. Rust panic with backtrace.
{
  const lines = ["$ cargo run --bin importer"];
  for (let i = 0; i < 80; i += 1) {
    lines.push("  waiting for upstream event stream...");
  }
  lines.push(
    "thread 'main' panicked at crates/importer/src/main.rs:42:9:",
    "called `Result::unwrap()` on an `Err` value: ConnectionRefused",
    "stack backtrace:",
    "   0: std::panicking::begin_panic_handler",
    "             at /rustc/0123456789abcdef/library\\std\\src\\panicking.rs:665:5",
    "   1: contextdrop_importer::connect",
    "             at ./crates/importer/src/main.rs:42:9",
    "   2: contextdrop_importer::main",
    "             at ./crates/importer/src/main.rs:88:5",
    "note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.",
  );
  addFixture(
    {
      id: "rust-panic",
      name: "Rust panic and backtrace",
      description:
        "A panicking importer with repeated wait output, panic message, and source-mapped backtrace frames.",
      file: "rust-panic.log",
      category: "Runtime failure",
      format: "terminal",
      must_preserve: [
        "thread 'main' panicked",
        "crates/importer/src/main.rs:42:9",
        "ConnectionRefused",
        "0: std::panicking::begin_panic_handler",
        "RUST_BACKTRACE=full",
      ],
      notes:
        "Repeated wait lines compress; panic text, error value, and backtrace frame locations are protected or adjacent to protected evidence.",
    },
    lines.join("\n"),
  );
}

// 10. Dense source text control: no deterministic redundancy.
{
  const lines = ["pub struct ContextWindow {"];
  for (let i = 1; i <= 120; i += 1) {
    lines.push(`    field_${String(i).padStart(3, "0")}: ConfigValue${i}, // stable configuration member ${i}`);
  }
  lines.push(
    "    pub retry_policy: RetryPolicy,",
    "}",
    "",
    "pub const MAX_IN_FLIGHT: usize = 32;",
    "pub const MAX_ATTEMPTS: usize = 7;",
    "pub const BASE_BACKOFF_MS: u64 = 250;",
    "pub const MAX_BACKOFF_MS: u64 = 30_000;",
    "",
    "impl ContextWindow {",
    "    pub fn retry_policy(&self) -> RetryPolicy {",
    "        RetryPolicy {",
    "            max_attempts: MAX_ATTEMPTS,",
    "            base_delay_ms: BASE_BACKOFF_MS,",
    "            max_delay_ms: MAX_BACKOFF_MS,",
    "        }",
    "    }",
    "}",
  );
  // Replace one field line with the stable assertion used by the manifest.
  lines[30] = "    retry_policy: ExponentialBackoff, // stable configuration member 29";
  addFixture(
    {
      id: "dense-source-control",
      name: "Dense source text control",
      description:
        "A dense, unique source-like file used to verify that ContextDrop does not invent savings.",
      file: "dense-source.txt",
      category: "Control",
      format: "plain-text",
      must_preserve: [
        "retry_policy: ExponentialBackoff",
        "MAX_IN_FLIGHT: usize = 32",
      ],
      notes:
        "Expected to return the original unchanged under all strategies. Useful as a no-false-savings control.",
    },
    lines.join("\n"),
  );
}

// 11. Go test failure with passing package summaries.
{
  const lines = ["$ go test ./... -v"];
  for (let i = 1; i <= 120; i += 1) {
    lines.push(`ok      example.com/ctxdrop/unit/module${String(i).padStart(3, "0")}    0.00${i % 9}s`);
  }
  lines.push(
    "=== RUN   TestCacheEvict",
    "--- FAIL: TestCacheEvict (0.01s)",
    "    cache_test.go:88: expected 2 entries, got 3",
    "FAIL",
    "exit status 1",
    "FAIL    example.com/ctxdrop/cache    0.021s",
  );
  addFixture(
    {
      id: "go-test-failure",
      name: "Go test failure",
      description:
        "A Go test run with 120 passing package summaries and one failing test with a source line.",
      file: "go-test-failure.log",
      category: "Test failure",
      format: "test-output",
      must_preserve: [
        "TestCacheEvict",
        "cache_test.go:88",
        "expected 2 entries, got 3",
        "exit status 1",
        "FAIL    example.com/ctxdrop/cache",
      ],
      notes:
        "Passing package lines are compressible. The failed test, source line, exit status, and failed package summary are protected.",
    },
    lines.join("\n"),
  );
}

// 12. Multilingual output with protected non-English errors.
{
  const lines = ["部署任务开始"];
  for (let i = 0; i < 120; i += 1) {
    lines.push("正在重试依赖下载，请稍候...");
  }
  lines.push(
    "",
    "错误: 数据库连接失败 (retry budget exhausted)",
    "エラー: 設定ファイルが見つかりません",
    "构建总结: 1 个错误, 2 个警告",
    "重试次数超过上限",
    "exit code: 1",
  );
  addFixture(
    {
      id: "multilingual-errors",
      name: "Multilingual errors",
      description:
        "Chinese and Japanese diagnostics surrounded by repeated Chinese progress text.",
      file: "multilingual-errors.log",
      category: "Unicode robustness",
      format: "plain-text",
      must_preserve: [
        "错误: 数据库连接失败",
        "エラー: 設定ファイルが見つかりません",
        "构建总结: 1 个错误, 2 个警告",
        "重试次数超过上限",
        "exit code: 1",
      ],
      notes:
        "Progress-like repeated Chinese text is compressible; non-English error markers are protected by the multilingual rules.",
    },
    lines.join("\n"),
  );
}

const manifest = {
  version: 1,
  fixtures,
};
writeFileSync(join(root, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`Generated ${fixtures.length} fixtures in ${root}`);
