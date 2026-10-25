import { useMemo } from "react";
import { diffLines, type Change } from "diff";
import { cn } from "../utils";

interface DiffRow {
  kind: "added" | "removed" | "context";
  text: string;
  oldNumber: number | null;
  newNumber: number | null;
}

const MAX_PREVIEW_ROWS = 1600;

function buildRows(changes: Change[]): DiffRow[] {
  const rows: DiffRow[] = [];
  let oldNumber = 1;
  let newNumber = 1;

  for (const change of changes) {
    const lines = change.value.split("\n");
    if (lines.at(-1) === "") {
      lines.pop();
    }
    for (const text of lines) {
      if (change.added) {
        rows.push({ kind: "added", text, oldNumber: null, newNumber });
        newNumber += 1;
      } else if (change.removed) {
        rows.push({ kind: "removed", text, oldNumber, newNumber: null });
        oldNumber += 1;
      } else {
        rows.push({ kind: "context", text, oldNumber, newNumber });
        oldNumber += 1;
        newNumber += 1;
      }
    }
  }
  return rows;
}

export function DiffView({ original, output }: { original: string; output: string }) {
  const { rows, added, removed } = useMemo(() => {
    const changes = diffLines(original, output);
    const built = buildRows(changes);
    return {
      rows: built,
      added: built.filter((row) => row.kind === "added").length,
      removed: built.filter((row) => row.kind === "removed").length,
    };
  }, [original, output]);

  const visibleRows = rows.slice(0, MAX_PREVIEW_ROWS);
  const truncated = rows.length > MAX_PREVIEW_ROWS;

  if (added === 0 && removed === 0) {
    return (
      <div className="border border-line bg-surface px-5 py-8 text-center">
        <p className="font-display text-2xl text-ink">No textual changes</p>
        <p className="mx-auto mt-2 max-w-md text-[13px] leading-5 text-muted">
          The engine returned the original input unchanged. This can happen when no qualifying
          deterministic redundancy was found or a safety fallback was used.
        </p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden border border-code-line bg-code text-code-text">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-code-line px-3 py-2 font-mono text-[10px] uppercase tracking-[0.08em] text-code-muted">
        <span className="flex items-center gap-4">
          <span className="text-[#e7a18a]">− {removed} removed</span>
          <span className="text-[#a9c7a7]">+ {added} added</span>
        </span>
        <span>real line diff</span>
      </div>
      <div className="code-scroll max-h-[34rem] overflow-auto" tabIndex={0} aria-label="Line diff between input and output">
        <div className="min-w-max py-1">
          {visibleRows.map((row, index) => (
            <div
              key={`${row.kind}-${index}`}
              className={cn(
                "flex min-h-[22px] items-start font-mono text-[12px] leading-[1.65]",
                row.kind === "added" && "bg-[#1e2b20] text-[#c6ddc4]",
                row.kind === "removed" && "bg-[#2c1f1b] text-[#e4b3a4]",
                row.kind === "context" && "text-code-text/70",
              )}
            >
              <span
                aria-hidden="true"
                className="w-12 shrink-0 select-none border-r border-code-line/60 px-2 text-right text-code-muted/60"
              >
                {row.oldNumber ?? ""}
              </span>
              <span
                aria-hidden="true"
                className="w-12 shrink-0 select-none border-r border-code-line/60 px-2 text-right text-code-muted/60"
              >
                {row.newNumber ?? ""}
              </span>
              <span
                aria-hidden="true"
                className={cn(
                  "w-6 shrink-0 select-none pl-2",
                  row.kind === "added" && "text-[#9ec79b]",
                  row.kind === "removed" && "text-[#d8917d]",
                )}
              >
                {row.kind === "added" ? "+" : row.kind === "removed" ? "−" : " "}
              </span>
              <span className="whitespace-pre pr-6">{row.text || " "}</span>
            </div>
          ))}
        </div>
      </div>
      {truncated ? (
        <p className="border-t border-code-line px-3 py-2 font-mono text-[10px] leading-4 text-code-muted">
          Diff preview limited to {MAX_PREVIEW_ROWS.toLocaleString("en-US")} of{" "}
          {rows.length.toLocaleString("en-US")} rows. Download the processed output for the complete
          result.
        </p>
      ) : null}
    </div>
  );
}
