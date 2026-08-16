import type { ButtonHTMLAttributes, CSSProperties, ReactNode } from "react";

export function StatusDot({
  tone = "muted",
  pulse = false,
  size = 7,
  label,
}: {
  label?: string;
  pulse?: boolean;
  size?: number;
  tone?: "ok" | "acc" | "warn" | "dang" | "muted" | "line";
}) {
  return (
    <span
      className={`status-dot tone-${tone}${pulse ? " is-pulse" : ""}`}
      style={{ width: size, height: size }}
      aria-label={label}
    />
  );
}

export function Badge({
  children,
  tone = "neutral",
  className = "",
}: {
  children: ReactNode;
  className?: string;
  tone?: "acc" | "ok" | "warn" | "dang" | "neutral" | "outline";
}) {
  return <em className={`ui-badge tone-${tone} ${className}`.trim()}>{children}</em>;
}

export function LatencyBadge({
  label,
  quality,
}: {
  label: string;
  quality: "good" | "ok" | "bad" | "fail" | null;
}) {
  const tone =
    quality === "good"
      ? "ok"
      : quality === "ok"
        ? "warn"
        : quality === "bad" || quality === "fail"
          ? "dang"
          : "muted";
  return (
    <span
      className={`latency-badge tone-${tone}${quality ? ` latency-${quality}` : ""}`}
      aria-label={quality ?? undefined}
    >
      {label}
    </span>
  );
}

export function SegmentedControl<T extends string>({
  ariaLabel,
  options,
  value,
  onChange,
  disabled,
}: {
  ariaLabel: string;
  disabled?: boolean;
  onChange: (value: T) => void;
  options: { label: ReactNode; tone?: string; value: T }[];
  value: T;
}) {
  return (
    <div className="segmented" role="group" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={option.value === value ? "is-on" : undefined}
          data-tone={option.tone}
          data-value={option.value}
          disabled={disabled}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Toggle({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <input
      aria-label={label}
      className="toggle"
      type="checkbox"
      checked={checked}
      disabled={disabled}
      onChange={(event) => onChange(event.target.checked)}
    />
  );
}

export function IconButton({
  label,
  children,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      type="button"
      className={`icon-btn ${className}`.trim()}
      aria-label={label}
      title={label}
      {...props}
    >
      {children}
    </button>
  );
}

export function Card({
  children,
  className = "",
  style,
}: {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <section className={`surface-card ${className}`.trim()} style={style}>
      {children}
    </section>
  );
}

export function StatCard({
  label,
  value,
  unit,
}: {
  label: string;
  unit?: string;
  value: string;
}) {
  return (
    <Card className="stat-card">
      <span className="field-label">{label}</span>
      <div className="stat-value">
        <strong>{value}</strong>
        {unit ? <span>{unit}</span> : null}
      </div>
    </Card>
  );
}

export function EmptyState({
  title,
  body,
  actions,
  label,
}: {
  actions?: ReactNode;
  body: ReactNode;
  label?: string;
  title: string;
}) {
  return (
    <div className="empty-state" aria-label={label ?? title}>
      <div className="empty-icon" aria-hidden />
      <h2>{title}</h2>
      <p>{body}</p>
      {actions ? <div className="empty-actions">{actions}</div> : null}
    </div>
  );
}