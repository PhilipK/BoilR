import clsx from "clsx";
import { useMemo, useState } from "react";

import type { PlatformSummary, ShortcutSummary } from "../types";
import { errorMessage, friendlyError, initials, plural, tileColor, toImageSrc } from "../lib/format";
import { Check, Toggle } from "./controls";
import { sourceStatus } from "./Sources";

/** Inline name editor: Enter saves, Escape cancels (without leaving the view). */
const RenameField = ({
  game,
  onSave,
  onCancel,
}: {
  game: ShortcutSummary;
  onSave: (name: string) => Promise<void>;
  onCancel: () => void;
}) => {
  const [name, setName] = useState(game.display_name);
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    try {
      await onSave(name);
    } finally {
      setSaving(false);
    }
  };
  return (
    <form
      className="flex items-center gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onCancel();
          }
        }}
        aria-label={`New name for ${game.app_name}`}
        placeholder={game.app_name}
        className="field min-w-0 flex-1 py-1"
      />
      <button type="submit" className="btn-primary py-1" disabled={saving}>
        {saving ? "Saving…" : "Save"}
      </button>
      <button type="button" className="link-btn" onClick={onCancel} disabled={saving}>
        Cancel
      </button>
    </form>
  );
};

const GameRow = ({
  game,
  selected,
  inSteam,
  onToggle,
  onRename,
  showSource,
}: {
  game: ShortcutSummary;
  selected: boolean;
  inSteam: boolean;
  onToggle: (value: boolean) => void;
  onRename: (name: string) => Promise<void>;
  showSource?: string;
}) => {
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const rename = async (name: string) => {
    setError(null);
    try {
      await onRename(name);
      setEditing(false);
    } catch (err) {
      setError(`Couldn't rename: ${errorMessage(err)}`);
    }
  };
  const renamed = game.display_name !== game.app_name;
  // Launchers often give an .exe as the icon (Steam extracts it); the web view can't show those.
  const [brokenIcon, setBrokenIcon] = useState<string | null>(null);
  const iconSrc = toImageSrc(game.icon);
  const icon = iconSrc !== brokenIcon ? iconSrc : null;
  return (
    <li
      className={clsx(
        "flex items-center gap-3 px-3 py-2 transition-colors hover:bg-harbour/50",
        !selected && "opacity-60"
      )}
    >
      <Check checked={selected} onChange={onToggle} label={`Import ${game.display_name}`} />
      {icon ? (
        <img
          src={icon}
          alt=""
          onError={() => setBrokenIcon(icon)}
          className="h-10 w-10 shrink-0 object-cover notch"
        />
      ) : (
        <span
          aria-hidden
          className={clsx("notch flex h-10 w-10 shrink-0 items-center justify-center font-pixel text-foam", tileColor(game.display_name))}
        >
          {initials(game.display_name)}
        </span>
      )}
      <div className="min-w-0 flex-1">
        {editing ? (
          <RenameField
            game={game}
            onSave={rename}
            onCancel={() => setEditing(false)}
          />
        ) : (
          <p className="truncate text-foam">{game.display_name}</p>
        )}
        {error ? (
          <p role="alert" className="text-sm text-ember">
            {error}
          </p>
        ) : renamed && !editing ? (
          <p className="truncate text-sm text-mauve">
            Renamed from {game.app_name}.{" "}
            <button type="button" className="link-btn" onClick={() => void rename(game.app_name)}>
              Use original name
            </button>
          </p>
        ) : (
          <p className="truncate text-sm text-mauve">{game.exe}</p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-3 text-sm">
        {editing ? null : (
          <button
            type="button"
            className="link-btn text-mauve decoration-mauve/50 hover:text-flame focus-visible:text-flame"
            onClick={() => setEditing(true)}
            aria-label={`Rename ${game.display_name}`}
          >
            Rename
          </button>
        )}
        {showSource ? <span className="text-peach">{showSource}</span> : null}
        {game.needs_proton ? <span className="text-mauve">Runs with Proton</span> : null}
        {selected && inSteam ? <span className="text-mauve">Already in Steam</span> : null}
      </div>
    </li>
  );
};

/** Launchers whose "games" are really apps; selections here need a human eye. */
const APP_SOURCES = new Set(["flatpak"]);

export const GameList = ({
  platforms,
  filter,
  isSelected,
  plannedAppIds,
  onToggleGame,
  onRenameGame,
  onSetMany,
  onTogglePlatform,
  busyPlatforms,
  onOpenSettings,
}: {
  platforms: PlatformSummary[];
  filter: string;
  isSelected: (appId: number) => boolean;
  plannedAppIds: Set<number>;
  onToggleGame: (appId: number, selected: boolean) => void;
  onRenameGame: (game: ShortcutSummary, name: string) => Promise<void>;
  onSetMany: (appIds: number[], selected: boolean) => void;
  onTogglePlatform: (codeName: string, enabled: boolean) => void;
  busyPlatforms: Set<string>;
  onOpenSettings: (codeName: string) => void;
}) => {
  const [query, setQuery] = useState("");
  const single = filter === "all" ? null : platforms.find((p) => p.code_name === filter) ?? null;
  const sources = single ? [single] : platforms.filter((p) => sourceStatus(p) === "games");

  const matches = (g: ShortcutSummary) =>
    !query || [g.display_name, g.app_name].some((n) => n.toLowerCase().includes(query.trim().toLowerCase()));

  const visible = useMemo(
    () => sources.flatMap((p) => p.games.filter(matches).map((g) => ({ game: g, source: p }))),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [sources, query]
  );
  const visibleIds = visible.map((v) => v.game.app_id);
  const selectedCount = visibleIds.filter(isSelected).length;

  const title = single ? single.name : "All games";
  const status = single ? sourceStatus(single) : "games";

  return (
    <section aria-labelledby="list-title" className="flex h-full min-h-0 flex-col">
      <header className="flex flex-wrap items-end justify-between gap-4 px-6 pb-4 pt-6">
        <div>
          <h1 id="list-title" className="font-pixel text-3xl text-foam">
            {title}
          </h1>
          {status === "games" && visibleIds.length + (query ? 1 : 0) > 0 ? (
            <p className="mt-1 text-mauve">
              {selectedCount} of {plural(visibleIds.length, "game")} selected for Steam
            </p>
          ) : null}
        </div>
        {single ? (
          <div className="flex items-center gap-3">
            <span className="text-sm text-peach">Include {single.name}</span>
            <Toggle
              checked={single.enabled}
              disabled={busyPlatforms.has(single.code_name)}
              onChange={(v) => onTogglePlatform(single.code_name, v)}
              label={`Include ${single.name}`}
            />
          </div>
        ) : null}
      </header>

      {status === "games" ? (
        <>
          {single && APP_SOURCES.has(single.code_name) ? (
            <p className="mx-6 mb-3 border-l-3 border-ember bg-harbour/50 px-4 py-2 text-sm text-peach">
              These are all your Flatpak apps, and not all of them are games. Untick the ones you
              don't want in your Steam library.
            </p>
          ) : null}
          <div className={clsx("flex flex-wrap items-center gap-3 px-6 pb-3", sources.every((p) => p.games.length === 0) && "hidden")}>
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Find a game"
              aria-label="Find a game"
              className="field max-w-xs py-1.5"
            />
            <button type="button" className="link-btn" onClick={() => onSetMany(visibleIds, true)}>
              Select all
            </button>
            <button type="button" className="link-btn" onClick={() => onSetMany(visibleIds, false)}>
              Select none
            </button>
          </div>
          <ul className="scroll-quiet min-h-0 flex-1 overflow-y-auto px-3 pb-6">
            {visible.length ? (
              visible.map(({ game, source }) => (
                <GameRow
                  key={`${source.code_name}-${game.app_id}`}
                  game={game}
                  selected={isSelected(game.app_id)}
                  inSteam={!plannedAppIds.has(game.app_id)}
                  onToggle={(v) => onToggleGame(game.app_id, v)}
                  onRename={(name) => onRenameGame(game, name)}
                  showSource={single ? undefined : source.name}
                />
              ))
            ) : query ? (
              <li className="px-3 py-8 text-mauve">No games match “{query}”.</li>
            ) : (
              <li className="max-w-prose px-3 py-8 text-peach">
                BoilR didn't find any games yet. The launchers on the left say what it looked for;
                pick one to see why, or point BoilR to it in Settings.
              </li>
            )}
          </ul>
        </>
      ) : single ? (
        <div className="max-w-prose px-6">
          {status === "off" ? (
            <p className="text-peach">{single.name} is turned off, so BoilR won't look for its games.</p>
          ) : status === "attention" ? (
            <>
              <p className="text-peach">
                BoilR found {single.name}, but couldn't read its games. If it's installed somewhere
                unusual, point BoilR to it in Settings.
              </p>
              <details className="mt-3 text-sm text-mauve">
                <summary className="cursor-pointer">Technical details</summary>
                <p className="mt-1 break-words">{single.error}</p>
              </details>
            </>
          ) : (
            <p className="text-peach">
              {single.error ? `${friendlyError(single.error)}.` : `${single.name} has no installed games.`}{" "}
              If you use {single.name} and installed it somewhere unusual, set its location in Settings.
            </p>
          )}
          <button type="button" className="btn-quiet mt-4" onClick={() => onOpenSettings(single.code_name)}>
            Open {single.name} settings
          </button>
        </div>
      ) : null}
    </section>
  );
};
