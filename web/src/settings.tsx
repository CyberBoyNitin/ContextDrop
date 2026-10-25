import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { InputFormat, Strategy, TokenizerKind } from "./types";

export const SETTINGS_STORAGE_KEY = "contextdrop.settings.v1";
export const SERVER_MAX_INPUT_BYTES = 1_000_000;

export const MAX_INPUT_OPTIONS = [
  { value: 100_000, label: "100 KB" },
  { value: 250_000, label: "250 KB" },
  { value: 500_000, label: "500 KB" },
  { value: 1_000_000, label: "1 MB" },
] as const;

export interface Settings {
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

export const DEFAULT_SETTINGS: Settings = {
  strategy: "balanced",
  tokenizer: "o200k_base",
  inputFormat: "auto",
  maxInputBytes: SERVER_MAX_INPUT_BYTES,
  collapseRepetitions: true,
  collapsePassingTests: true,
  collapseProgressUpdates: true,
  structuralJson: true,
  preservePatterns: [],
  pricingEnabled: false,
  pricePerMillion: "",
  currency: "USD",
};

const STRATEGIES: Strategy[] = ["conservative", "balanced", "aggressive"];
const TOKENIZERS: TokenizerKind[] = ["o200k_base", "cl100k_base", "estimate"];
const FORMATS: InputFormat[] = [
  "auto",
  "plain-text",
  "terminal",
  "build-log",
  "test-output",
  "json",
  "structured-log",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function pickEnum<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && allowed.includes(value as T) ? (value as T) : fallback;
}

function pickBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function validPattern(pattern: string): boolean {
  return (
    pattern.length > 0 &&
    pattern.length <= 256 &&
    !/[\r\n\0]/.test(pattern)
  );
}

export function sanitizeSettings(value: unknown): Settings {
  if (!isRecord(value)) {
    return { ...DEFAULT_SETTINGS, preservePatterns: [] };
  }

  const maxInput =
    typeof value.maxInputBytes === "number" &&
    Number.isInteger(value.maxInputBytes) &&
    value.maxInputBytes >= 1 &&
    value.maxInputBytes <= SERVER_MAX_INPUT_BYTES
      ? value.maxInputBytes
      : DEFAULT_SETTINGS.maxInputBytes;

  const patterns = Array.isArray(value.preservePatterns)
    ? value.preservePatterns
        .filter((pattern): pattern is string => typeof pattern === "string")
        .map((pattern) => pattern.trim())
        .filter(validPattern)
        .slice(0, 100)
    : [];

  const uniquePatterns = [...new Set(patterns)];

  const price =
    typeof value.pricePerMillion === "string" && value.pricePerMillion.length <= 32
      ? value.pricePerMillion
      : "";

  const currency =
    typeof value.currency === "string" && /^[A-Za-z0-9]{3,8}$/.test(value.currency)
      ? value.currency.toUpperCase()
      : DEFAULT_SETTINGS.currency;

  return {
    strategy: pickEnum(value.strategy, STRATEGIES, DEFAULT_SETTINGS.strategy),
    tokenizer: pickEnum(value.tokenizer, TOKENIZERS, DEFAULT_SETTINGS.tokenizer),
    inputFormat: pickEnum(value.inputFormat, FORMATS, DEFAULT_SETTINGS.inputFormat),
    maxInputBytes: maxInput,
    collapseRepetitions: pickBoolean(
      value.collapseRepetitions,
      DEFAULT_SETTINGS.collapseRepetitions,
    ),
    collapsePassingTests: pickBoolean(
      value.collapsePassingTests,
      DEFAULT_SETTINGS.collapsePassingTests,
    ),
    collapseProgressUpdates: pickBoolean(
      value.collapseProgressUpdates,
      DEFAULT_SETTINGS.collapseProgressUpdates,
    ),
    structuralJson: pickBoolean(value.structuralJson, DEFAULT_SETTINGS.structuralJson),
    preservePatterns: uniquePatterns,
    pricingEnabled: pickBoolean(value.pricingEnabled, DEFAULT_SETTINGS.pricingEnabled),
    pricePerMillion: price,
    currency,
  };
}

export function validateSettings(settings: Settings): string[] {
  const errors: string[] = [];
  if (
    !Number.isInteger(settings.maxInputBytes) ||
    settings.maxInputBytes < 1 ||
    settings.maxInputBytes > SERVER_MAX_INPUT_BYTES
  ) {
    errors.push(
      `Maximum input must be between 1 and ${SERVER_MAX_INPUT_BYTES.toLocaleString("en-US")} bytes.`,
    );
  }
  if (settings.preservePatterns.length > 100) {
    errors.push("At most 100 custom preserve patterns are supported.");
  }
  for (const pattern of settings.preservePatterns) {
    if (!validPattern(pattern)) {
      errors.push("Each custom pattern must be 1–256 characters with no line breaks.");
      break;
    }
  }
  if (settings.pricingEnabled) {
    const price = Number(settings.pricePerMillion);
    if (
      settings.pricePerMillion.trim() === "" ||
      !Number.isFinite(price) ||
      price < 0 ||
      price > 1_000_000
    ) {
      errors.push("Input token price must be a number between 0 and 1,000,000.");
    }
    if (!/^[A-Z0-9]{3,8}$/.test(settings.currency.trim().toUpperCase())) {
      errors.push("Currency must be a 3–8 character code such as USD or EUR.");
    }
  }
  return errors;
}

function loadSettings(): { settings: Settings; persisted: boolean } {
  if (typeof window === "undefined") {
    return { settings: { ...DEFAULT_SETTINGS, preservePatterns: [] }, persisted: false };
  }
  try {
    const raw = window.localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return { settings: { ...DEFAULT_SETTINGS, preservePatterns: [] }, persisted: true };
    }
    return { settings: sanitizeSettings(JSON.parse(raw)), persisted: true };
  } catch {
    return { settings: { ...DEFAULT_SETTINGS, preservePatterns: [] }, persisted: false };
  }
}

interface SettingsContextValue {
  settings: Settings;
  saveSettings: (settings: Settings) => void;
  resetSettings: () => void;
  persisted: boolean;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [initial] = useState(loadSettings);
  const [settings, setSettings] = useState<Settings>(initial.settings);
  const [persisted, setPersisted] = useState(initial.persisted);

  const saveSettings = useCallback((next: Settings) => {
    const sanitized = sanitizeSettings(next);
    setSettings(sanitized);
    try {
      window.localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(sanitized));
      setPersisted(true);
    } catch {
      setPersisted(false);
    }
  }, []);

  const resetSettings = useCallback(() => {
    setSettings({ ...DEFAULT_SETTINGS, preservePatterns: [] });
    try {
      window.localStorage.removeItem(SETTINGS_STORAGE_KEY);
      setPersisted(true);
    } catch {
      setPersisted(false);
    }
  }, []);

  useEffect(() => {
    function handleStorage(event: StorageEvent) {
      if (event.key !== SETTINGS_STORAGE_KEY) {
        return;
      }
      try {
        setSettings(
          event.newValue
            ? sanitizeSettings(JSON.parse(event.newValue))
            : { ...DEFAULT_SETTINGS, preservePatterns: [] },
        );
        setPersisted(true);
      } catch {
        setPersisted(false);
      }
    }
    window.addEventListener("storage", handleStorage);
    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  const value = useMemo(
    () => ({ settings, saveSettings, resetSettings, persisted }),
    [settings, saveSettings, resetSettings, persisted],
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error("useSettings must be used inside SettingsProvider");
  }
  return context;
}
