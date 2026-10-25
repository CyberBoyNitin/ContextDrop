import type {
  ApiErrorEnvelope,
  BenchmarkRequest,
  BenchmarkRun,
  CompressionRequest,
  CompressionResult,
  FixtureDetail,
  FixtureSummary,
  HealthResponse,
  InspectRequest,
  ProtectionInspection,
  RuleCatalog,
} from "./types";

const API_BASE = (import.meta.env.VITE_API_BASE ?? "/api").replace(/\/$/, "");

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...init,
      credentials: "omit",
      cache: "no-store",
      headers: {
        Accept: "application/json",
        ...(init.body ? { "Content-Type": "application/json" } : {}),
        ...init.headers,
      },
    });
  } catch (error) {
    if (isAbortError(error)) {
      throw error;
    }
    throw new ApiError(
      0,
      "network_error",
      "The local ContextDrop API could not be reached. Start contextdrop-server and try again.",
    );
  }

  const raw = await response.text();
  let parsed: unknown = null;
  if (raw) {
    try {
      parsed = JSON.parse(raw) as unknown;
    } catch {
      parsed = null;
    }
  }

  if (!response.ok) {
    const envelope = parsed as ApiErrorEnvelope | null;
    throw new ApiError(
      response.status,
      envelope?.error?.code ?? `http_${response.status}`,
      envelope?.error?.message ?? `The local API returned HTTP ${response.status}.`,
    );
  }
  if (parsed === null) {
    throw new ApiError(response.status, "invalid_response", "The local API returned an empty response.");
  }
  return parsed as T;
}

function postJson<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  return request<T>(path, {
    method: "POST",
    body: JSON.stringify(body),
    signal,
  });
}

export function compressInput(
  payload: CompressionRequest,
  signal?: AbortSignal,
): Promise<CompressionResult> {
  return postJson<CompressionResult>("/compress", payload, signal);
}

export function runBenchmark(
  payload: BenchmarkRequest,
  signal?: AbortSignal,
): Promise<BenchmarkRun> {
  return postJson<BenchmarkRun>("/benchmark", payload, signal);
}

export function inspectInput(
  payload: InspectRequest,
  signal?: AbortSignal,
): Promise<ProtectionInspection> {
  return postJson<ProtectionInspection>("/inspect", payload, signal);
}

export function getFixtures(signal?: AbortSignal): Promise<FixtureSummary[]> {
  return request<FixtureSummary[]>("/fixtures", { signal });
}

export function getFixture(id: string, signal?: AbortSignal): Promise<FixtureDetail> {
  return request<FixtureDetail>(`/fixtures/${encodeURIComponent(id)}`, { signal });
}

export function getRules(signal?: AbortSignal): Promise<RuleCatalog> {
  return request<RuleCatalog>("/rules", { signal });
}

export function getHealth(signal?: AbortSignal): Promise<HealthResponse> {
  return request<HealthResponse>("/health", { signal });
}

export function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "An unexpected local error occurred.";
}
