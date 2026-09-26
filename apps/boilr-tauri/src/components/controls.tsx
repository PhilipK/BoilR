import clsx from "clsx";
import type { ReactNode } from "react";

/** On/off switch drawn as a pixel lever. Wraps a real checkbox for keyboard and screen readers. */
export const Toggle = ({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  disabled?: boolean;
}) => (
  <label className={clsx("relative inline-flex shrink-0", disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer")}>
    <input
      type="checkbox"
      role="switch"
      className="peer sr-only"
      aria-label={label}
      checked={checked}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
    />
    <span className="notch h-6 w-11 bg-harbour transition-colors peer-checked:bg-ember peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-flame" />
    <span className="absolute left-1 top-1 h-4 w-4 bg-mauve transition-transform peer-checked:translate-x-5 peer-checked:bg-foam" />
  </label>
);

/** Square pixel checkbox. */
export const Check = ({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  disabled?: boolean;
}) => (
  <label className={clsx("relative inline-flex h-6 w-6 shrink-0 items-center justify-center", disabled ? "cursor-not-allowed" : "cursor-pointer")}>
    <input
      type="checkbox"
      className="peer sr-only"
      aria-label={label}
      checked={checked}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
    />
    <span className="h-5 w-5 border-2 border-mauve bg-deep transition-colors peer-checked:border-flame peer-checked:bg-flame peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-flame peer-disabled:opacity-40" />
    {/* Pixel tick: one square per pixel on a 7x5 grid. */}
    <svg
      aria-hidden
      viewBox="0 0 7 5"
      shapeRendering="crispEdges"
      className="pointer-events-none absolute hidden h-[10px] w-[14px] fill-deep peer-checked:block"
    >
      {[[6, 0], [5, 1], [0, 2], [4, 2], [1, 3], [3, 3], [2, 4]].map(([x, y]) => (
        <rect key={`${x}-${y}`} x={x} y={y} width="1" height="1" />
      ))}
    </svg>
  </label>
);

/** A labelled row with a control on the right, used throughout Settings. */
export const SettingRow = ({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: ReactNode;
}) => (
  <div className="flex items-start justify-between gap-6 py-3">
    <div className="max-w-prose">
      <p className="font-bold text-foam">{title}</p>
      {hint ? <p className="mt-0.5 text-sm text-mauve">{hint}</p> : null}
    </div>
    {children}
  </div>
);
