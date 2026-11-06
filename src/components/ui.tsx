import {
  useEffect,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type ReactNode,
  type Ref,
} from "react";
import { createPortal } from "react-dom";
import { useEscape } from "../lib/hooks";
import { RISK_LABEL, RISK_TONE } from "../lib/risk";
import type { Risk } from "../lib/types";
import {
  IconAlert,
  IconCheck,
  IconChevronDown,
  IconClose,
  IconInfo,
  IconLock,
  IconSearch,
  type IconProps,
} from "./Icons";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not(:disabled)",
  "input:not(:disabled):not([type='hidden'])",
  "select:not(:disabled)",
  "textarea:not(:disabled)",
  "[tabindex]",
].join(",");

function focusableWithin(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) =>
      element.tabIndex >= 0 &&
      !element.matches(":disabled") &&
      !element.closest("[inert], [aria-hidden='true']") &&
      (element.getClientRects().length > 0 || element === document.activeElement),
  );
}

let drawerLockCount = 0;
let bodyOverflowBeforeDrawer = "";

export function isDrawerOpen(): boolean {
  return drawerLockCount > 0;
}

function lockBodyScroll(): () => void {
  if (drawerLockCount === 0) {
    bodyOverflowBeforeDrawer = document.body.style.overflow;
  }
  drawerLockCount += 1;
  document.body.style.overflow = "hidden";
  window.dispatchEvent(new CustomEvent("cyron:drawer-change", { detail: { open: true } }));

  let released = false;
  return () => {
    if (released) return;
    released = true;
    drawerLockCount = Math.max(0, drawerLockCount - 1);
    if (drawerLockCount === 0) {
      document.body.style.overflow = bodyOverflowBeforeDrawer;
      bodyOverflowBeforeDrawer = "";
      window.dispatchEvent(new CustomEvent("cyron:drawer-change", { detail: { open: false } }));
    }
  };
}

/* ---------------------------------------------------------------- buttons */

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  loading?: boolean;
  icon?: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  icon,
  className,
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cx("btn", `btn--${variant}`, size === "sm" && "btn--sm", className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Spinner size={13} /> : icon}
      {children}
    </button>
  );
}

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  children: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

export function IconButton({ label, className, children, ...rest }: IconButtonProps) {
  return (
    <button
      type="button"
      className={cx("icon-btn", className)}
      aria-label={label}
      title={label}
      {...rest}
    >
      {children}
    </button>
  );
}

/* ----------------------------------------------------------------- badges */

export type BadgeTone = "neutral" | "safe" | "caution" | "danger" | "accent";

export function Badge({ tone = "neutral", children }: { tone?: BadgeTone; children: ReactNode }) {
  return <span className={cx("badge", `badge--${tone}`)}>{children}</span>;
}

export function RiskBadge({ risk }: { risk: Risk }) {
  return (
    <span className={cx("badge", `badge--${RISK_TONE[risk]}`)}>
      {risk === "protected" ? <IconLock size={11} /> : <span className="badge__dot" />}
      {RISK_LABEL[risk]}
    </span>
  );
}

export function Eyebrow({ children, className }: { children: ReactNode; className?: string }) {
  return <span className={cx("eyebrow", className)}>{children}</span>;
}

/* ----------------------------------------------------------------- panels */

interface PanelProps {
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
  flush?: boolean;
}

export function Panel({ title, subtitle, actions, children, className, bodyClassName, flush }: PanelProps) {
  return (
    <section className={cx("panel", className)}>
      {(title || actions) && (
        <header className="panel__head">
          <div className="panel__heading">
            {title && <h2 className="panel__title">{title}</h2>}
            {subtitle && <p className="panel__subtitle">{subtitle}</p>}
          </div>
          {actions && <div className="panel__actions">{actions}</div>}
        </header>
      )}
      <div className={cx("panel__body", flush && "panel__body--flush", bodyClassName)}>{children}</div>
    </section>
  );
}

interface ViewHeaderProps {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
  meta?: ReactNode;
}

export function ViewHeader({ title, description, actions, meta }: ViewHeaderProps) {
  return (
    <header className="view-header">
      <div className="view-header__text">
        <div className="view-header__title-row">
          <h1>{title}</h1>
          {meta}
        </div>
        {description && <p className="view-header__description">{description}</p>}
      </div>
      {actions && <div className="view-header__actions">{actions}</div>}
    </header>
  );
}

/* ------------------------------------------------------------------ stats */

interface StatProps {
  label: ReactNode;
  value: ReactNode;
  meta?: ReactNode;
  size?: "md" | "lg";
  tone?: "default" | "accent";
}

export function Stat({ label, value, meta, size = "md", tone = "default" }: StatProps) {
  return (
    <div className={cx("stat", `stat--${size}`, tone === "accent" && "stat--accent")}>
      <span className="stat__label">{label}</span>
      <span className="stat__value">{value}</span>
      {meta && <span className="stat__meta">{meta}</span>}
    </div>
  );
}

/* ---------------------------------------------------------------- controls */

interface SearchFieldProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  className?: string;
}

export function SearchField({ value, onChange, placeholder, ariaLabel, className }: SearchFieldProps) {
  return (
    <div className={cx("search", className)}>
      <IconSearch size={15} className="search__icon" />
      <input
        type="search"
        className="search__input"
        value={value}
        placeholder={placeholder}
        aria-label={ariaLabel ?? placeholder ?? "Search"}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && value) {
            event.stopPropagation();
            onChange("");
          }
        }}
      />
      {value && (
        <IconButton label="Clear search" className="search__clear" onClick={() => onChange("")}>
          <IconClose size={13} />
        </IconButton>
      )}
    </div>
  );
}

interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  count?: number;
}

interface SegmentedProps<T extends string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  ariaLabel: string;
  size?: "sm" | "md";
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  ariaLabel,
  size = "md",
}: SegmentedProps<T>) {
  return (
    <div className={cx("segmented", size === "sm" && "segmented--sm")} role="group" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className="segmented__item"
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
          {option.count !== undefined && (
            <span className="segmented__count" aria-hidden="true">
              {option.count}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
}

export function Toggle({ checked, onChange, label, description }: ToggleProps) {
  const labelId = useId();
  const descriptionId = useId();
  return (
    <div className="toggle-row">
      <div className="toggle-row__text">
        <span className="toggle-row__label" id={labelId}>
          {label}
        </span>
        {description && (
          <p className="toggle-row__description" id={descriptionId}>
            {description}
          </p>
        )}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={labelId}
        aria-describedby={description ? descriptionId : undefined}
        className="switch"
        onClick={() => onChange(!checked)}
      >
        <span className="switch__thumb" aria-hidden="true" />
      </button>
    </div>
  );
}

export function Select({
  value,
  onChange,
  options,
  ariaLabel,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  ariaLabel: string;
  className?: string;
}) {
  return (
    <div className={cx("select", className)}>
      <select
        value={value}
        aria-label={ariaLabel}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <IconChevronDown size={14} className="select__chevron" />
    </div>
  );
}

interface FieldProps {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  htmlFor?: string;
}

export function Field({ label, hint, children, htmlFor }: FieldProps) {
  return (
    <div className="field">
      <label className="field__label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
      {hint && <p className="field__hint">{hint}</p>}
    </div>
  );
}

/* --------------------------------------------------------------- progress */

export function ProgressBar({
  value,
  tone = "accent",
  label,
}: {
  value?: number;
  tone?: "accent" | "safe" | "caution" | "danger";
  label?: string;
}) {
  const indeterminate = value === undefined;
  const percent = indeterminate ? undefined : Math.round(Math.min(Math.max(value, 0), 1) * 100);
  return (
    <div
      className={cx("progress", `progress--${tone}`, indeterminate && "progress--indeterminate")}
      role="progressbar"
      aria-label={label ?? "Progress"}
      aria-valuemin={indeterminate ? undefined : 0}
      aria-valuemax={indeterminate ? undefined : 100}
      aria-valuenow={percent}
      aria-valuetext={indeterminate ? "In progress" : percent === undefined ? undefined : `${percent}%`}
      aria-busy={indeterminate || undefined}
    >
      <span
        className="progress__fill"
        style={indeterminate ? undefined : { width: `${Math.min(Math.max(value, 0), 1) * 100}%` }}
      />
    </div>
  );
}

export function Spinner({ size = 16 }: { size?: number }) {
  return <span className="spinner" style={{ width: size, height: size }} aria-hidden="true" />;
}

export function Skeleton({ width, height = 12 }: { width?: number | string; height?: number }) {
  return <span className="skeleton" style={{ width, height }} aria-hidden="true" />;
}

/* ----------------------------------------------------------- empty states */

interface EmptyStateProps {
  icon?: (props: IconProps) => ReactNode;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}

export function EmptyState({ icon: Icon, title, description, action, compact }: EmptyStateProps) {
  return (
    <div className={cx("empty", compact && "empty--compact")}>
      {Icon && (
        <span className="empty__icon">
          <Icon size={compact ? 18 : 22} />
        </span>
      )}
      <h3 className="empty__title">{title}</h3>
      {description && <p className="empty__description">{description}</p>}
      {action && <div className="empty__action">{action}</div>}
    </div>
  );
}

/* ---------------------------------------------------------------- banners */

export function Banner({
  tone = "info",
  children,
  actions,
}: {
  tone?: "info" | "caution" | "danger" | "accent" | "safe";
  children: ReactNode;
  actions?: ReactNode;
}) {
  const Icon =
    tone === "caution" || tone === "danger" ? IconAlert : tone === "safe" ? IconCheck : IconInfo;
  return (
    <div className={cx("banner", `banner--${tone}`)}>
      <Icon size={16} className="banner__icon" />
      <div className="banner__text">{children}</div>
      {actions && <div className="banner__actions">{actions}</div>}
    </div>
  );
}

/* ----------------------------------------------------------------- drawer */

interface DrawerProps {
  open: boolean;
  title: ReactNode;
  subtitle?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}

export function Drawer({ open, title, subtitle, onClose, children, footer }: DrawerProps) {
  useEscape(open, onClose);
  const headingId = useId();
  const subtitleId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!open) return;
    const restoreTarget =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const unlockBodyScroll = lockBodyScroll();
    closeRef.current?.focus();

    const trapTab = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const panel = panelRef.current;
      if (!panel) return;
      const focusable = focusableWithin(panel);
      if (focusable.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      const outside = !(active instanceof Node) || !panel.contains(active);
      if (event.shiftKey && (outside || active === first)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (outside || active === last)) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", trapTab);
    return () => {
      document.removeEventListener("keydown", trapTab);
      unlockBodyScroll();
      // A list row can disappear while the drawer is open. Never call focus() on a
      // detached element; when it still exists, return focus to the exact trigger.
      if (restoreTarget?.isConnected) {
        restoreTarget.focus();
      } else {
        // If the originating row was removed during the modal interaction, move
        // focus to a stable application control rather than leaving it in the void.
        document.querySelector<HTMLElement>(".sidebar__brand")?.focus();
      }
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div className="drawer-layer">
      <div className="drawer__scrim" onClick={onClose} aria-hidden="true" />
      <aside
        ref={panelRef}
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby={headingId}
        aria-describedby={subtitle ? subtitleId : undefined}
        tabIndex={-1}
      >
        <header className="drawer__head">
          <div>
            <h2 id={headingId} className="drawer__title">
              {title}
            </h2>
            {subtitle && (
              <p id={subtitleId} className="drawer__subtitle">
                {subtitle}
              </p>
            )}
          </div>
          <IconButton ref={closeRef} label="Close" onClick={onClose}>
            <IconClose size={16} />
          </IconButton>
        </header>
        <div className="drawer__body">{children}</div>
        {footer && <footer className="drawer__foot">{footer}</footer>}
      </aside>
    </div>,
    document.body,
  );
}

/* ---------------------------------------------------------------- details */

export function KeyValue({ items }: { items: { label: string; value: ReactNode }[] }) {
  return (
    <dl className="kv">
      {items.map((item) => (
        <div className="kv__row" key={item.label}>
          <dt>{item.label}</dt>
          <dd>{item.value ?? "—"}</dd>
        </div>
      ))}
    </dl>
  );
}
