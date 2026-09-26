import clsx from "clsx";
import { useState } from "react";

import type { PlatformSummary } from "../types";
import { friendlyError, isNotInstalledError } from "../lib/format";

export type SourceStatus = "games" | "attention" | "empty" | "off";

export const sourceStatus = (p: PlatformSummary): SourceStatus => {
  if (!p.enabled) return "off";
  if (p.games.length > 0) return "games";
  if (p.error && !isNotInstalledError(p.error)) return "attention";
  return "empty";
};

/** "all" or a platform code_name. */
export type SourceFilter = string;

const Row = ({
  active,
  onClick,
  name,
  detail,
  count,
  tone = "normal",
}: {
  active: boolean;
  onClick: () => void;
  name: string;
  detail?: string;
  count?: string;
  tone?: "normal" | "warn" | "muted";
}) => (
  <button
    type="button"
    onClick={onClick}
    aria-current={active ? "true" : undefined}
    className={clsx(
      "group flex w-full items-center gap-3 border-l-3 px-4 py-2 text-left transition-colors",
      active ? "border-flame bg-harbour" : "border-transparent hover:bg-harbour/60"
    )}
  >
    <span className="min-w-0 flex-1">
      <span
        className={clsx(
          "block truncate",
          tone === "muted" ? "text-mauve" : active ? "text-foam" : "text-peach"
        )}
      >
        {name}
      </span>
      {detail ? (
        <span className={clsx("block truncate text-sm", tone === "warn" ? "text-ember" : "text-mauve")}>
          {detail}
        </span>
      ) : null}
    </span>
    {count ? <span className="shrink-0 font-pixel text-flame">{count}</span> : null}
  </button>
);

const GroupTitle = ({ children }: { children: string }) => (
  <h2 className="px-4 pb-1 pt-5 font-pixel text-sm text-mauve">{children}</h2>
);

export const Sources = ({
  platforms,
  selectedIds,
  filter,
  onFilter,
}: {
  platforms: PlatformSummary[];
  selectedIds: (p: PlatformSummary) => number;
  filter: SourceFilter;
  onFilter: (f: SourceFilter) => void;
}) => {
  const [showQuiet, setShowQuiet] = useState(false);
  const withGames = platforms.filter((p) => sourceStatus(p) === "games");
  const attention = platforms.filter((p) => sourceStatus(p) === "attention");
  const empty = platforms.filter((p) => sourceStatus(p) === "empty");
  const off = platforms.filter((p) => sourceStatus(p) === "off");
  const totalGames = withGames.reduce((n, p) => n + p.games.length, 0);
  const totalSelected = withGames.reduce((n, p) => n + selectedIds(p), 0);

  return (
    <nav aria-label="Game sources" className="scroll-quiet flex h-full flex-col overflow-y-auto pb-6">
      <GroupTitle>Your launchers</GroupTitle>
      <Row
        active={filter === "all"}
        onClick={() => onFilter("all")}
        name="All games"
        count={`${totalSelected}/${totalGames}`}
      />
      {withGames.map((p) => (
        <Row
          key={p.code_name}
          active={filter === p.code_name}
          onClick={() => onFilter(p.code_name)}
          name={p.name}
          count={`${selectedIds(p)}/${p.games.length}`}
        />
      ))}

      {attention.length ? (
        <>
          <GroupTitle>Needs a look</GroupTitle>
          {attention.map((p) => (
            <Row
              key={p.code_name}
              active={filter === p.code_name}
              onClick={() => onFilter(p.code_name)}
              name={p.name}
              detail={friendlyError(p.error ?? "")}
              tone="warn"
            />
          ))}
        </>
      ) : null}

      {empty.length || off.length ? (
        <>
          <button
            type="button"
            onClick={() => setShowQuiet((v) => !v)}
            aria-expanded={showQuiet}
            className="mt-5 px-4 text-left font-pixel text-sm text-mauve hover:text-peach"
          >
            {showQuiet ? "▾" : "▸"} Nothing found ({empty.length + off.length})
          </button>
          {showQuiet ? (
            <>
              {empty.map((p) => (
                <Row
                  key={p.code_name}
                  active={filter === p.code_name}
                  onClick={() => onFilter(p.code_name)}
                  name={p.name}
                  detail={p.error ? friendlyError(p.error) : "No games installed"}
                  tone="muted"
                />
              ))}
              {off.map((p) => (
                <Row
                  key={p.code_name}
                  active={filter === p.code_name}
                  onClick={() => onFilter(p.code_name)}
                  name={p.name}
                  detail="Turned off"
                  tone="muted"
                />
              ))}
            </>
          ) : (
            <p className="px-4 pt-1 text-sm text-mauve">
              {[...empty, ...off].map((p) => p.name).join(", ")}
            </p>
          )}
        </>
      ) : null}
    </nav>
  );
};
