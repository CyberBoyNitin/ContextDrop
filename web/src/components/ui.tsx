import type {
  ButtonHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from "react";
import { AlertTriangle, ChevronDown, type LucideIcon } from "lucide-react";
import { cn } from "../utils";

type ButtonVariant = "primary" | "secondary" | "ghost" | "accent" | "danger";
type ButtonSize = "sm" | "md" | "icon";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

const buttonVariants: Record<ButtonVariant, string> = {
  primary:
    "bg-ink text-paper border border-ink hover:bg-rust hover:border-rust active:bg-rust-dark active:border-rust-dark",
  secondary:
    "bg-surface text-ink border border-line-strong hover:border-ink hover:bg-paper-deep/60 active:bg-paper-deep",
  ghost:
    "bg-transparent text-ink-soft border border-transparent hover:bg-paper-deep/75 hover:text-ink active:bg-paper-deep",
  accent:
    "bg-rust text-white border border-rust hover:bg-rust-dark hover:border-rust-dark active:bg-rust-dark",
  danger:
    "bg-transparent text-danger border border-danger/40 hover:border-danger hover:bg-danger-soft active:bg-danger-soft",
};

const buttonSizes: Record<ButtonSize, string> = {
  sm: "min-h-9 px-3 py-1.5 text-[13px] gap-2",
  md: "min-h-11 px-4 py-2.5 text-sm gap-2.5",
  icon: "h-10 w-10 p-0 justify-center",
};

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  className,
  children,
  disabled,
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex select-none items-center justify-center rounded-[2px] font-medium tracking-[0.005em] transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rust disabled:pointer-events-none disabled:opacity-45",
        buttonVariants[variant],
        buttonSizes[size],
        className,
      )}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...props}
    >
      {loading ? (
        <span
          aria-hidden="true"
          className="inline-block h-3.5 w-3.5 shrink-0 animate-spin rounded-full border-2 border-current border-r-transparent"
        />
      ) : null}
      {children}
    </button>
  );
}

type BadgeTone = "neutral" | "success" | "warning" | "danger" | "accent" | "code";

const badgeTones: Record<BadgeTone, string> = {
  neutral: "border-line-strong bg-paper-deep/60 text-ink-soft",
  success: "border-moss/25 bg-moss-soft text-moss",
  warning: "border-amber/30 bg-amber-soft text-amber-dark",
  danger: "border-danger/25 bg-danger-soft text-danger",
  accent: "border-rust/25 bg-rust-soft text-rust-dark",
  code: "border-code-line bg-code text-code-text",
};

export function Badge({
  children,
  tone = "neutral",
  dot = false,
  className,
}: {
  children: ReactNode;
  tone?: BadgeTone;
  dot?: boolean;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-[2px] border px-2 py-0.5 font-mono text-[10px] font-medium uppercase tracking-[0.1em]",
        badgeTones[tone],
        className,
      )}
    >
      {dot ? <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-current" /> : null}
      {children}
    </span>
  );
}

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  hideLabel?: boolean;
  hint?: string;
  containerClassName?: string;
}

export function Select({
  label,
  hideLabel = false,
  hint,
  className,
  containerClassName,
  children,
  disabled,
  ...props
}: SelectProps) {
  return (
    <label className={cn("block", containerClassName)}>
      <span
        className={cn(
          "mb-1.5 block font-mono text-[10px] font-medium uppercase tracking-[0.12em] text-ink-soft",
          hideLabel && "sr-only",
        )}
      >
        {label}
      </span>
      <span className="relative block">
        <select
          className={cn(
            "field-input h-10 w-full appearance-none pr-9 disabled:cursor-not-allowed disabled:opacity-50",
            className,
          )}
          disabled={disabled}
          {...props}
        >
          {children}
        </select>
        <ChevronDown
          aria-hidden="true"
          className="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-soft"
        />
      </span>
      {hint ? <span className="mt-1.5 block text-xs leading-relaxed text-muted">{hint}</span> : null}
    </label>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}) {
  return (
    <label
      className={cn(
        "group flex cursor-pointer items-start gap-3 rounded-[2px] py-2.5",
        disabled && "cursor-not-allowed opacity-50",
      )}
    >
      <input
        type="checkbox"
        className="peer sr-only"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span
        aria-hidden="true"
        className="toggle-track mt-0.5 group-hover:border-ink/60"
      >
        <span className="toggle-knob" />
      </span>
      <span className="min-w-0">
        <span className="block text-sm font-medium leading-5 text-ink">{label}</span>
        {description ? (
          <span className="mt-0.5 block text-[13px] leading-5 text-muted">{description}</span>
        ) : null}
      </span>
    </label>
  );
}

export function PageHeader({
  index,
  title,
  description,
  actions,
  meta,
}: {
  index: string;
  title: string;
  description: string;
  actions?: ReactNode;
  meta?: ReactNode;
}) {
  return (
    <header className="mb-8 border-b border-line pb-7 lg:mb-10 lg:pb-8">
      <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
        <div className="min-w-0">
          <div className="mb-4 flex items-center gap-3 font-mono text-[10px] uppercase tracking-[0.16em] text-ink-soft">
            <span aria-hidden="true" className="h-2 w-2 bg-rust" />
            <span>ContextDrop / {index}</span>
          </div>
          <h1 className="font-display text-[clamp(2.9rem,5.2vw,4.8rem)] font-normal leading-[0.9] tracking-[-0.025em] text-ink">
            {title}
          </h1>
          <p className="mt-4 max-w-2xl text-[15px] leading-6 text-ink-soft lg:text-base lg:leading-7">
            {description}
          </p>
        </div>
        {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
      {meta ? (
        <div className="mt-6 flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-line/70 pt-3">
          {meta}
        </div>
      ) : null}
    </header>
  );
}

export function SectionHeading({
  index,
  title,
  description,
  actions,
}: {
  index?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
      <div className="flex min-w-0 items-start gap-3">
        {index ? (
          <span className="mt-1 font-mono text-[10px] font-medium tracking-[0.12em] text-rust">
            {index}
          </span>
        ) : null}
        <div>
          <h2 className="text-[15px] font-semibold tracking-[-0.01em] text-ink">{title}</h2>
          {description ? (
            <p className="mt-1 max-w-2xl text-[13px] leading-5 text-muted">{description}</p>
          ) : null}
        </div>
      </div>
      {actions ? <div className="shrink-0">{actions}</div> : null}
    </div>
  );
}

export function ErrorNotice({
  message,
  title = "Something went wrong",
  onRetry,
  retryLabel = "Try again",
}: {
  message: string;
  title?: string;
  onRetry?: () => void;
  retryLabel?: string;
}) {
  return (
    <div
      role="alert"
      className="flex items-start gap-3 border border-danger/30 bg-danger-soft/70 px-3.5 py-3 text-sm text-danger"
    >
      <AlertTriangle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="font-medium">{title}</p>
        <p className="mt-0.5 break-words leading-5 text-danger/85">{message}</p>
      </div>
      {onRetry ? (
        <Button variant="danger" size="sm" onClick={onRetry} className="shrink-0">
          {retryLabel}
        </Button>
      ) : null}
    </div>
  );
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-12 text-center">
      <span className="mb-5 grid h-11 w-11 place-items-center border border-line-strong bg-paper-deep/40 text-ink-soft">
        <Icon aria-hidden="true" className="h-5 w-5" strokeWidth={1.5} />
      </span>
      <h3 className="font-display text-2xl leading-none text-ink">{title}</h3>
      <p className="mt-3 max-w-md text-[13px] leading-6 text-muted">{description}</p>
      {children ? <div className="mt-5">{children}</div> : null}
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded-[2px] border border-line-strong bg-paper px-1.5 py-0.5 font-mono text-[10px] font-normal text-ink-soft">
      {children}
    </kbd>
  );
}
