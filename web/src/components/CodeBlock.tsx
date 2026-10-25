import type { ReactNode } from "react";
import { cn } from "../utils";

export function CodeBlock({
  value,
  ariaLabel,
  className,
  maxHeight = "28rem",
  header,
  footer,
}: {
  value: string;
  ariaLabel: string;
  className?: string;
  maxHeight?: string;
  header?: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className={cn("border border-code-line bg-code text-code-text", className)}>
      {header ? (
        <div className="flex min-h-10 items-center justify-between gap-3 border-b border-code-line px-3 py-2">
          {header}
        </div>
      ) : null}
      <pre
        tabIndex={0}
        aria-label={ariaLabel}
        style={{ maxHeight }}
        className="code-scroll overflow-auto whitespace-pre p-4 font-mono text-[12px] leading-[1.75] text-code-text/90 focus-visible:outline focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-rust"
      >
        {value || " "}
      </pre>
      {footer ? (
        <div className="border-t border-code-line px-3 py-2 font-mono text-[10px] text-code-muted">
          {footer}
        </div>
      ) : null}
    </div>
  );
}
