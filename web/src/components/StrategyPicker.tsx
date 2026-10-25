import { Check } from "lucide-react";
import type { Strategy } from "../types";
import { cn, strategyDescription, strategyLabel } from "../utils";

const TAGLINES: Record<Strategy, string> = {
  conservative: "Minimal / contiguous",
  balanced: "Default / contextual",
  aggressive: "Deep / global",
};

export function StrategyPicker({
  value,
  onChange,
  showDescriptions = false,
  disabled = false,
  name = "strategy",
}: {
  value: Strategy;
  onChange: (strategy: Strategy) => void;
  showDescriptions?: boolean;
  disabled?: boolean;
  name?: string;
}) {
  const options: Strategy[] = ["conservative", "balanced", "aggressive"];

  return (
    <fieldset disabled={disabled}>
      <legend className="sr-only">Compression strategy</legend>
      <div className="grid gap-2 sm:grid-cols-3">
        {options.map((strategy) => {
          const selected = value === strategy;
          return (
            <label
              key={strategy}
              className={cn(
                "strategy-option group relative flex cursor-pointer flex-col border px-3 py-2.5 transition-colors",
                selected
                  ? "border-ink bg-ink text-paper"
                  : "border-line-strong bg-surface text-ink hover:border-ink/60 hover:bg-paper-deep/40",
                disabled && "cursor-not-allowed opacity-60",
              )}
            >
              <input
                className="sr-only"
                type="radio"
                name={name}
                value={strategy}
                checked={selected}
                disabled={disabled}
                onChange={() => onChange(strategy)}
              />
              <span className="flex items-center justify-between gap-2">
                <span className="text-[13px] font-semibold tracking-[-0.01em]">
                  {strategyLabel(strategy)}
                </span>
                {selected ? (
                  <Check aria-hidden="true" className="h-3.5 w-3.5 text-rust-soft" />
                ) : (
                  <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full border border-current opacity-30" />
                )}
              </span>
              <span
                className={cn(
                  "mt-0.5 font-mono text-[9px] uppercase tracking-[0.12em]",
                  selected ? "text-paper/55" : "text-muted",
                )}
              >
                {TAGLINES[strategy]}
              </span>
              {showDescriptions ? (
                <span
                  className={cn(
                    "mt-2.5 block text-[12px] leading-[1.55]",
                    selected ? "text-paper/70" : "text-muted",
                  )}
                >
                  {strategyDescription(strategy)}
                </span>
              ) : null}
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
