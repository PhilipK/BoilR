import clsx from "clsx";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Settings } from "../settings";
import type { ArtworkGame, ArtworkKind, ArtworkOption, GameMatch, SteamAccount } from "../types";
import { errorMessage, initials, plural, tileColor, toImageSrc } from "../lib/format";

const KINDS: { kind: ArtworkKind; label: string; hint: string }[] = [
  { kind: "grid", label: "Cover", hint: "Portrait art in your library." },
  {
    kind: "hero",
    label: "Banner",
    hint: "The wide image at the top of the game's page.",
  },
  { kind: "logo", label: "Logo", hint: "Sits on top of the banner." },
  {
    kind: "wide_grid",
    label: "Wide cover",
    hint: "Used on the Recent Games shelf.",
  },
  {
    kind: "icon",
    label: "Icon",
    hint: "The small icon in lists and the taskbar.",
  },
  {
    kind: "big_picture",
    label: "Big Picture",
    hint: "Wide art for Big Picture and the Steam Deck.",
  },
];
const labelOf = (kind: ArtworkKind) => KINDS.find((k) => k.kind === kind)?.label ?? kind;

/** Columns sized to each kind's shape, so options are compared at a useful size. */
const OPTION_GRID: Record<ArtworkKind, string> = {
  grid: "grid-cols-[repeat(auto-fill,minmax(8rem,1fr))]",
  wide_grid: "grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]",
  big_picture: "grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]",
  hero: "grid-cols-[repeat(auto-fill,minmax(20rem,1fr))]",
  logo: "grid-cols-[repeat(auto-fill,minmax(12rem,1fr))]",
  icon: "grid-cols-[repeat(auto-fill,minmax(5rem,1fr))]",
};
const ASPECT: Record<ArtworkKind, string> = {
  grid: "aspect-[2/3]",
  wide_grid: "aspect-[920/430]",
  big_picture: "aspect-[920/430]",
  hero: "aspect-[1920/620]",
  logo: "aspect-[16/9]",
  icon: "aspect-square",
};

/** Logos and icons are often transparent: show them on a pixel checkerboard. */
const CHECKER =
  "bg-[length:16px_16px] bg-[linear-gradient(45deg,theme(colors.harbour)_25%,transparent_25%,transparent_75%,theme(colors.harbour)_75%),linear-gradient(45deg,theme(colors.harbour)_25%,transparent_25%,transparent_75%,theme(colors.harbour)_75%)] bg-[position:0_0,8px_8px] bg-deep";

const imageSrc = (game: ArtworkGame, kind: ArtworkKind) => {
  const image = game.images[kind];
  return image ? toImageSrc(image.path, image.version) : null;
};

const Placeholder = ({ name, className }: { name: string; className?: string }) => (
  <div className={clsx("flex flex-col items-center justify-center gap-2", tileColor(name), className)}>
    <span className="font-pixel text-3xl text-foam">{initials(name)}</span>
  </div>
);

/** A local image over its placeholder: the placeholder shows while it loads, and stays if the file is broken. */
const Art = ({
  src,
  name,
  alt,
  fit = "cover",
}: {
  src: string | null;
  name: string;
  alt: string;
  fit?: "cover" | "contain";
}) => {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  return (
    <div className="relative h-full w-full">
      <Placeholder name={name} className="absolute inset-0" />
      {src && !failed ? (
        <img
          src={src}
          alt={alt}
          loading="lazy"
          decoding="async"
          onError={() => setFailed(true)}
          className={clsx("relative h-full w-full", fit === "cover" ? "object-cover" : "object-contain")}
        />
      ) : null}
    </div>
  );
};

// ---------------------------------------------------------------------------------------------
// No key yet

const KeyOnboarding = ({ onSaved }: { onSaved: (s: Settings) => void }) => {
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    try {
      onSaved(
        await invoke<Settings>("update_settings", {
          update: { steamgrid_db: { auth_key: key.trim() } },
        }),
      );
    } catch (err) {
      setError(errorMessage(err));
    }
  };
  return (
    <div className="max-w-prose">
      <p className="text-peach">
        BoilR gets covers, banners and logos from SteamGridDB, a free community site. It needs a key to ask for them.
      </p>
      <ol className="mt-3 list-decimal space-y-1 pl-5 text-peach">
        <li>Sign in at steamgriddb.com (a Steam login works).</li>
        <li>Open Preferences, then API, and copy your key.</li>
        <li>Paste it here.</li>
      </ol>
      <div className="mt-4 flex gap-2">
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Paste your key"
          aria-label="SteamGridDB key"
          className="field max-w-sm flex-1"
        />
        <button type="button" className="btn-primary" disabled={!key.trim()} onClick={save}>
          Save key
        </button>
      </div>
      {error ? <p className="mt-2 text-ember">{error}</p> : null}
    </div>
  );
};

// ---------------------------------------------------------------------------------------------
// One game

const Preview = ({ game }: { game: ArtworkGame }) => {
  const hero = imageSrc(game, "hero");
  const logo = imageSrc(game, "logo");
  const cover = imageSrc(game, "grid");
  return (
    <div className="flex gap-4" aria-label={`How ${game.name} looks in Steam`}>
      <div className="aspect-[2/3] w-36 shrink-0 overflow-hidden notch">
        <Art src={cover} name={game.name} alt={`${game.name} cover`} />
      </div>
      <div className="relative min-w-0 flex-1 overflow-hidden notch">
        <div className="aspect-[1920/620] w-full">
          {hero ? (
            <img src={hero} alt={`${game.name} banner`} className="h-full w-full object-cover" />
          ) : (
            <div className="h-full w-full bg-harbour" />
          )}
        </div>
        <div className="absolute bottom-3 left-4 flex h-1/2 w-2/5 items-end">
          {logo ? (
            <img
              src={logo}
              alt={`${game.name} logo`}
              className="max-h-full max-w-full object-contain object-left-bottom"
            />
          ) : (
            <span className="font-pixel text-2xl text-foam drop-shadow-[2px_2px_0_theme(colors.deep)]">
              {game.name}
            </span>
          )}
        </div>
      </div>
    </div>
  );
};

const GameMatchPanel = ({ game, onMatched }: { game: ArtworkGame; onMatched: (name: string) => void }) => {
  const [match, setMatch] = useState<GameMatch | null>(null);
  const [query, setQuery] = useState(game.name);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const search = useCallback(
    async (q?: string) => {
      setBusy(true);
      setError(null);
      try {
        setMatch(
          await invoke<GameMatch>("artwork_game_match", {
            appId: game.app_id,
            name: game.name,
            query: q ?? null,
          }),
        );
      } catch (err) {
        setError(errorMessage(err));
      } finally {
        setBusy(false);
      }
    },
    [game],
  );

  useEffect(() => {
    void search();
  }, [search]);

  const choose = async (id: number, name: string) => {
    setBusy(true);
    try {
      await invoke("set_artwork_game", {
        appId: game.app_id,
        name: game.name,
        gridId: id,
      });
      onMatched(name);
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <div className="mt-3 border-l-3 border-ember bg-harbour/40 px-4 py-3">
      <p className="text-peach">Pick the game SteamGridDB should use for {game.name}.</p>
      <form
        className="mt-3 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void search(query);
        }}
      >
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Search SteamGridDB"
          className="field max-w-sm flex-1 py-1.5"
        />
        <button type="submit" className="btn-quiet" disabled={busy}>
          Search
        </button>
      </form>
      {error ? <p className="mt-2 text-ember">{error}</p> : null}
      <ul className="mt-3 space-y-1">
        {match?.candidates.map((c) => (
          <li key={c.id}>
            <button
              type="button"
              disabled={busy}
              onClick={() => choose(c.id, c.name)}
              className={clsx(
                "flex w-full items-center justify-between gap-3 px-3 py-2 text-left hover:bg-harbour",
                c.id === match.current_id && "bg-harbour",
              )}
            >
              <span className="text-foam">
                {c.name}
                {c.year ? <span className="text-mauve"> ({c.year})</span> : null}
              </span>
              {c.id === match.current_id ? <span className="shrink-0 text-sm text-flame">Used now</span> : null}
            </button>
          </li>
        ))}
        {match && match.candidates.length === 0 ? (
          <li className="text-mauve">No games found. Try a shorter name.</li>
        ) : null}
      </ul>
    </div>
  );
};

const GameDetail = ({
  game,
  userId,
  onBack,
  onChanged,
}: {
  game: ArtworkGame;
  userId: string;
  onBack: () => void;
  onChanged: (game: ArtworkGame) => void;
}) => {
  const [kind, setKind] = useState<ArtworkKind>("grid");
  const [options, setOptions] = useState<ArtworkOption[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [applying, setApplying] = useState<number | null>(null);
  const [matching, setMatching] = useState(false);
  const [reload, setReload] = useState(0);
  const barRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [chosen, setChosen] = useState<Partial<Record<ArtworkKind, number>>>({});
  const never = game.never_download.includes(kind);
  const current = imageSrc(game, kind);

  useEffect(() => {
    let live = true;
    setOptions(null);
    setError(null);
    invoke<ArtworkOption[]>("artwork_options", {
      appId: game.app_id,
      name: game.name,
      kindName: kind,
    })
      .then((o) => live && setOptions(o))
      .catch((err) => live && setError(errorMessage(err)));
    return () => {
      live = false;
    };
  }, [game.app_id, game.name, kind, reload]);

  const apply = async (option: ArtworkOption) => {
    setApplying(option.id);
    setNotice(null);
    setError(null);
    try {
      const image = await invoke<ArtworkGame["images"][ArtworkKind]>("set_artwork", {
        userId,
        appId: game.app_id,
        name: game.name,
        kindName: kind,
        url: option.url,
        extension: option.extension,
      });
      onChanged({
        ...game,
        images: { ...game.images, [kind]: image },
        never_download: game.never_download.filter((k) => k !== kind),
      });
      setChosen((c) => ({ ...c, [kind]: option.id }));
      setNotice(`${labelOf(kind)} updated. Restart Steam to see it.`);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setApplying(null);
    }
  };

  const clear = async (neverDownload: boolean) => {
    setError(null);
    try {
      await invoke("clear_artwork", {
        userId,
        appId: game.app_id,
        kindName: kind,
        neverDownload,
      });
      const images = { ...game.images };
      delete images[kind];
      const rest = game.never_download.filter((k) => k !== kind);
      onChanged({
        ...game,
        images,
        never_download: neverDownload ? [...rest, kind] : rest,
      });
      setChosen((c) => ({ ...c, [kind]: undefined }));
      setNotice(
        neverDownload
          ? `BoilR won't download a ${labelOf(kind).toLowerCase()} for ${game.name}.`
          : `${labelOf(kind)} removed.`,
      );
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const allowAgain = async () => {
    try {
      await invoke("clear_artwork", {
        userId,
        appId: game.app_id,
        kindName: kind,
        neverDownload: false,
      });
      onChanged({
        ...game,
        never_download: game.never_download.filter((k) => k !== kind),
      });
      setNotice(null);
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const hint = KINDS.find((k) => k.kind === kind)?.hint;

  /** After switching kind deep in a list, start the new list from its top rather than mid-way. */
  const showListTop = () => {
    const bar = barRef.current?.getBoundingClientRect();
    const list = listRef.current?.getBoundingClientRect();
    const scroller = barRef.current?.closest(".overflow-y-auto");
    if (bar && list && scroller && list.top < bar.bottom) scroller.scrollTop -= bar.bottom - list.top;
  };

  return (
    <div className="mx-auto max-w-5xl px-6 py-6">
      <button type="button" className="link-btn" onClick={onBack}>
        All artwork
      </button>
      <h1 className="mt-3 font-pixel text-3xl text-foam">{game.name}</h1>
      <div className="mt-4">
        <Preview game={game} />
      </div>

      {/* Stays in view while scrolling through options, so the result of a pick is always visible. */}
      <div ref={barRef} className="sticky top-0 z-10 -mx-2 mt-4 bg-night px-2 pb-3 pt-2">
        <div role="tablist" aria-label="Kind of artwork" className="flex flex-wrap gap-2">
          {KINDS.map((k) => (
            <button
              key={k.kind}
              type="button"
              role="tab"
              aria-selected={kind === k.kind}
              onClick={() => {
                setKind(k.kind);
                setMatching(false);
                setNotice(null);
                showListTop();
              }}
              className={clsx(
                "notch px-3 py-1.5 font-pixel",
                kind === k.kind ? "bg-flame text-deep" : "bg-harbour text-peach hover:bg-dusk",
              )}
            >
              {k.label}
              <span
                className={clsx(
                  "ml-2 text-sm",
                  kind === k.kind ? "text-deep" : game.images[k.kind] ? "text-flame" : "text-mauve",
                )}
              >
                {game.images[k.kind] ? "set" : game.never_download.includes(k.kind) ? "off" : "none"}
              </span>
            </button>
          ))}
        </div>

        <div className="mt-4 flex flex-wrap items-start justify-between gap-4">
          <p className="max-w-prose text-mauve">{hint}</p>
          <div className="flex flex-wrap gap-4">
            {current ? (
              <button type="button" className="link-btn" onClick={() => clear(false)}>
                Remove
              </button>
            ) : null}
            {never ? null : (
              <button type="button" className="link-btn" onClick={() => clear(true)}>
                Never download this
              </button>
            )}
            <button type="button" className="link-btn" onClick={() => setMatching((m) => !m)}>
              {matching ? "Close" : "Wrong game?"}
            </button>
          </div>
        </div>

        {matching ? (
          <GameMatchPanel
            game={game}
            onMatched={(name) => {
              setMatching(false);
              setNotice(`Now using SteamGridDB's ${name}.`);
              setReload((r) => r + 1);
            }}
          />
        ) : null}
        {notice ? (
          <p role="status" className="mt-3 border-l-3 border-flame px-4 py-2 text-foam">
            {notice}
          </p>
        ) : null}
        {error ? (
          <p role="alert" className="mt-3 border-l-3 border-ember px-4 py-2 text-peach">
            {error}
          </p>
        ) : null}
      </div>
      {never ? (
        <p className="mt-3 text-peach">
          BoilR won't download a {labelOf(kind).toLowerCase()} for this game.{" "}
          <button type="button" className="link-btn" onClick={allowAgain}>
            Allow again
          </button>
        </p>
      ) : null}

      <div ref={listRef} className="mt-4">
        {options === null && !error ? (
          <p className="text-mauve">Asking SteamGridDB…</p>
        ) : options && options.length === 0 ? (
          <p className="max-w-prose text-peach">
            SteamGridDB has no {labelOf(kind).toLowerCase()} for this game. If it matched the wrong game, use “Wrong
            game?”.
          </p>
        ) : (
          <ul className={clsx("grid gap-3", OPTION_GRID[kind])}>
            {options?.map((option) => (
              <li key={option.id}>
                <button
                  type="button"
                  onClick={() => apply(option)}
                  disabled={applying !== null}
                  aria-label={`Use this ${labelOf(kind).toLowerCase()} by ${option.author}`}
                  className={clsx(
                    "group relative block w-full overflow-hidden notch outline-offset-[-3px] hover:outline hover:outline-3 hover:outline-flame",
                    chosen[kind] === option.id && "outline outline-3 outline-flame",
                    ASPECT[kind],
                    kind === "logo" || kind === "icon" ? CHECKER : "bg-harbour",
                  )}
                >
                  <img
                    src={option.thumb}
                    alt=""
                    loading="lazy"
                    className={clsx(
                      "h-full w-full",
                      kind === "logo" || kind === "icon" ? "object-contain p-2" : "object-cover",
                    )}
                  />
                  {chosen[kind] === option.id ? (
                    <span className="absolute bottom-0 left-0 bg-flame px-2 py-0.5 font-pixel text-sm text-deep">
                      In use
                    </span>
                  ) : null}
                  {applying === option.id ? (
                    <span className="absolute inset-0 flex items-center justify-center bg-deep/70 font-pixel text-foam">
                      Applying…
                    </span>
                  ) : null}
                </button>
                <p className="mt-1 truncate text-sm text-mauve">
                  {option.width}×{option.height}, by {option.author}
                </p>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------------------------
// All games

export const ArtworkView = ({
  settings,
  onSettingsChanged,
}: {
  settings: Settings | null;
  onSettingsChanged: (settings: Settings) => void;
}) => {
  const hasKey = Boolean(settings?.steamgrid_db?.auth_key?.trim());
  const [accounts, setAccounts] = useState<SteamAccount[] | null>(null);
  const [userId, setUserId] = useState<string | null>(null);
  const [games, setGames] = useState<ArtworkGame[] | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [missingOnly, setMissingOnly] = useState(false);
  const [finding, setFinding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<SteamAccount[]>("list_steam_accounts")
      .then((a) => {
        setAccounts(a);
        setUserId((current) => current ?? a[0]?.user_id ?? null);
      })
      .catch((err) => setError(errorMessage(err)));
  }, []);

  const load = useCallback(async () => {
    if (!userId) return;
    try {
      setGames(await invoke<ArtworkGame[]>("list_artwork", { userId }));
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [userId]);

  useEffect(() => {
    void load();
  }, [load]);

  const findMissing = async () => {
    setFinding(true);
    setError(null);
    try {
      await invoke("find_missing_artwork");
      await load();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setFinding(false);
    }
  };

  const withCover = games?.filter((g) => g.images.grid).length ?? 0;
  const shown = useMemo(() => (games ?? []).filter((g) => !missingOnly || !g.images.grid), [games, missingOnly]);
  const game = games?.find((g) => g.app_id === selected) ?? null;

  if (game && userId) {
    return (
      <div className="scroll-quiet h-full overflow-y-auto">
        <GameDetail
          game={game}
          userId={userId}
          onBack={() => setSelected(null)}
          onChanged={(g) => setGames((all) => all?.map((x) => (x.app_id === g.app_id ? g : x)) ?? null)}
        />
      </div>
    );
  }

  return (
    <div className="scroll-quiet h-full overflow-y-auto">
      <div className="mx-auto max-w-6xl px-6 py-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="font-pixel text-3xl text-foam">Artwork</h1>
            {games && hasKey ? (
              <p className="mt-1 text-mauve">
                {withCover} of {plural(games.length, "shortcut")} have a cover
              </p>
            ) : null}
          </div>
          {hasKey ? (
            <div className="flex flex-wrap items-center gap-4">
              {accounts && accounts.length > 1 ? (
                <label className="flex items-center gap-2 text-sm text-peach">
                  Steam account
                  <select
                    value={userId ?? ""}
                    onChange={(e) => setUserId(e.target.value)}
                    className="field w-auto py-1"
                  >
                    {accounts.map((a) => (
                      <option key={a.user_id} value={a.user_id}>
                        {a.user_id} ({plural(a.shortcut_count, "shortcut")})
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}
              <button
                type="button"
                className="link-btn"
                onClick={() => setMissingOnly((m) => !m)}
                aria-pressed={missingOnly}
              >
                {missingOnly ? "Show all" : "Missing a cover only"}
              </button>
              <button type="button" className="btn-quiet" onClick={findMissing} disabled={finding}>
                {finding ? "Finding artwork…" : "Find missing artwork"}
              </button>
            </div>
          ) : null}
        </div>

        {error ? (
          <p role="alert" className="mt-4 border-l-3 border-ember px-4 py-2 text-peach">
            {error}
          </p>
        ) : null}

        <div className="mt-6">
          {!hasKey ? (
            <KeyOnboarding onSaved={onSettingsChanged} />
          ) : accounts && accounts.length === 0 ? (
            <p className="max-w-prose text-peach">
              No Steam shortcuts yet. Import some games first, then choose their artwork here.
            </p>
          ) : games === null ? (
            <p className="text-mauve">Loading…</p>
          ) : shown.length === 0 ? (
            <p className="text-peach">Every shortcut has a cover.</p>
          ) : (
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-4">
              {shown.map((g) => {
                const cover = imageSrc(g, "grid");
                return (
                  <li key={g.app_id}>
                    <button
                      type="button"
                      onClick={() => setSelected(g.app_id)}
                      className="group block w-full text-left"
                    >
                      <div className="aspect-[2/3] overflow-hidden notch outline-offset-[-3px] group-hover:outline group-hover:outline-3 group-hover:outline-flame">
                        <Art src={cover} name={g.name} alt="" />
                      </div>
                      <p className="mt-2 truncate text-foam">{g.name}</p>
                      <p className="text-sm text-mauve">
                        {cover ? `${Object.keys(g.images).length} of 6 images` : "No cover yet"}
                      </p>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
};
