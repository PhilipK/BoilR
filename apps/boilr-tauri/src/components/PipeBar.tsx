import clsx from "clsx";
import { useState } from "react";

import boiler from "../assets/boiler.png";
import steam from "../assets/steam.png";
import type { RemovalPlan, SyncOutcome, SyncProgressEvent } from "../types";
import { plural } from "../lib/format";

const progressText = (p: SyncProgressEvent): string => {
  switch (p.state) {
    case "starting":
      return "Heating up…";
    case "found_games":
      return `Piping ${plural(p.games_found, "game")} into Steam…`;
    case "finding_images":
      return "Looking for artwork…";
    case "downloading_images":
      return `Downloading ${plural(p.to_download, "cover")}…`;
    case "done":
      return "Done";
    case "error":
      return `Import failed: ${p.message}`;
    default:
      return "Starting…";
  }
};

/**
 * The boiler, the pipe to Steam and the one action that matters: import.
 * Puffs of steam travel along the pipe while a sync runs.
 */
export const PipeBar = ({
  toAdd,
  removals,
  syncing,
  progress,
  outcome,
  syncError,
  restartsSteam,
  hasGames,
  padConnected,
  onImport,
}: {
  toAdd: number;
  removals: RemovalPlan[];
  syncing: boolean;
  progress: SyncProgressEvent;
  outcome: SyncOutcome | null;
  syncError: string | null;
  restartsSteam: boolean;
  hasGames: boolean;
  padConnected: boolean;
  onImport: () => void;
}) => {
  const [showRemovals, setShowRemovals] = useState(false);
  const nothingToDo = toAdd === 0 && removals.length === 0;

  let status: string;
  if (syncing) {
    status = progressText(progress);
  } else if (syncError) {
    status = `Import failed: ${syncError}`;
  } else if (outcome && nothingToDo) {
    status = restartsSteam
      ? "Your games are in Steam."
      : "Your games are in Steam. Restart Steam to see them.";
  } else if (nothingToDo && !hasGames) {
    status = "No games to import yet.";
  } else if (nothingToDo) {
    status = "Everything selected is already in Steam.";
  } else {
    status = `${plural(toAdd, "new game")} ready for Steam`;
  }

  const buttonLabel = syncing ? "Importing…" : nothingToDo ? "Nothing to import" : `Import ${plural(toAdd, "game")}`;

  return (
    <footer className="relative border-t-3 border-harbour bg-deep">
      {showRemovals && removals.length ? (
        <div className="absolute bottom-full right-4 mb-2 w-96 max-w-[calc(100vw-2rem)] border-2 border-harbour bg-deep p-4 notch">
          <p className="font-pixel text-foam">Old shortcuts BoilR will remove</p>
          <p className="mt-1 text-sm text-mauve">
            Leftovers from earlier BoilR versions or duplicates. Your own shortcuts are never touched.
          </p>
          <ul className="scroll-quiet mt-3 max-h-48 space-y-1 overflow-y-auto text-sm">
            {removals.map((r) => (
              <li key={`${r.user_id}-${r.shortcut.app_id}`} className="flex justify-between gap-3">
                <span className="truncate text-peach">{r.shortcut.display_name || r.shortcut.app_name}</span>
                <span className="shrink-0 text-mauve">{r.reason === "duplicate_app_id" ? "duplicate" : "old version"}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className="mx-auto flex max-w-screen-xl items-center gap-4 px-5 py-3">
        <img
          src={boiler}
          alt=""
          className={clsx("pixelated h-20 w-20 shrink-0", syncing && "simmer")}
        />

        <div className="min-w-0 flex-1">
          <p
            role="status"
            className={clsx("truncate font-pixel text-lg", syncError ? "text-ember" : "text-foam")}
          >
            {status}
          </p>
          {/* The pipe: flanged at both ends, riveted along its length. */}
          <div
            className="relative mt-2 flex items-center"
            style={{ ["--pipe-length" as string]: "calc(100% - 4.5rem)" }}
          >
            <span aria-hidden className="h-7 w-2 shrink-0 bg-ember" />
            <div className="pipe h-4 flex-1 border-y-3 border-ember" />
            <span aria-hidden className="h-7 w-2 shrink-0 bg-ember" />
            {syncing
              ? [0, 0.6, 1.2].map((delay) => (
                  <span
                    key={delay}
                    aria-hidden
                    className="puff absolute left-3 top-0 h-3 w-3 bg-foam"
                    style={{ animationDelay: `${delay}s` }}
                  />
                ))
              : null}
            <img src={steam} alt="Steam" className="pixelated -my-3 ml-1 h-12 w-12 shrink-0" />
          </div>
          {removals.length ? (
            <button type="button" className="link-btn mt-1" onClick={() => setShowRemovals((v) => !v)}>
              {showRemovals ? "Hide" : "Also removes"} {plural(removals.length, "old shortcut")}
            </button>
          ) : null}
        </div>

        {padConnected ? (
          <dl aria-label="Controller buttons" className="hidden shrink-0 grid-cols-[auto_auto] items-center gap-x-2 gap-y-1 text-sm text-mauve lg:grid">
            {[
              ["A", "Select"],
              ["B", "Back"],
              ["Y", "Import"],
              ["L1 R1", "Switch tab"],
            ].map(([key, what]) => (
              <div key={key} className="contents">
                <dt className="notch justify-self-end bg-harbour px-1.5 font-pixel text-peach">{key}</dt>
                <dd>{what}</dd>
              </div>
            ))}
          </dl>
        ) : null}
        <button
          id="import-button"
          type="button"
          className="btn-primary shrink-0 px-6 py-3 text-lg"
          onClick={onImport}
          disabled={syncing || nothingToDo}
        >
          {buttonLabel}
        </button>
      </div>
    </footer>
  );
};
