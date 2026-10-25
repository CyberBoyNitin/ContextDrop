import { useEffect, useRef, useState } from "react";
import { Check, Copy, TriangleAlert } from "lucide-react";
import { copyText } from "../utils";
import { Button } from "./ui";

export function CopyButton({
  value,
  label = "Copy",
  copiedLabel = "Copied",
  className,
  compact = false,
}: {
  value: string;
  label?: string;
  copiedLabel?: string;
  className?: string;
  compact?: boolean;
}) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    return () => {
      if (timer.current !== undefined) {
        window.clearTimeout(timer.current);
      }
    };
  }, []);

  async function handleCopy() {
    const copied = await copyText(value);
    setState(copied ? "copied" : "failed");
    if (timer.current !== undefined) {
      window.clearTimeout(timer.current);
    }
    timer.current = window.setTimeout(() => setState("idle"), 2200);
  }

  const text = state === "copied" ? copiedLabel : state === "failed" ? "Copy failed" : label;
  const Icon = state === "copied" ? Check : state === "failed" ? TriangleAlert : Copy;

  if (compact) {
    return (
      <Button
        variant="ghost"
        size="icon"
        onClick={handleCopy}
        className={className}
        aria-label={text}
        title={text}
        disabled={!value}
        type="button"
      >
        <Icon aria-hidden="true" className="h-4 w-4" />
      </Button>
    );
  }

  return (
    <Button
      variant="secondary"
      size="sm"
      onClick={handleCopy}
      className={className}
      aria-label={text}
      type="button"
      disabled={!value}
    >
      <Icon aria-hidden="true" className="h-3.5 w-3.5" />
      <span aria-live="polite">{text}</span>
    </Button>
  );
}
