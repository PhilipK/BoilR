import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import clsx from "clsx";

import logo from "./assets/logo32.png";
import type {
  PlatformSettingsPayload,
  PlatformSummary,
  PlatformToggleResponse,
  SettingsUpdatePayload,
  SyncOutcome,
  SyncPlan,
  SyncProgressEvent,
} from "./types";
import type { Settings } from "./settings";
import { errorMessage } from "./lib/format";
import {
  buildPlatformSettingsGroup,
  buildPlatformSettingsPayload,
  mergeOptionalFieldKinds,
  resetGroup,
} from "./lib/platformSettings";
import type { PlatformFieldUpdate, PlatformSettingsGroup } from "./lib/platformSettings";
import { GameList } from "./components/GameList";
import { PipeBar } from "./components/PipeBar";
import { Sources } from "./components/Sources";
import { SettingsView } from "./components/SettingsView";
import { useNavigation } from "./lib/navigation";

type View = "games" | "settings";

const applySettingsPatch = (current: Settings | null, patch: SettingsUpdatePayload): Settings | null => {
  if (!current) return current;
  return {
    ...current,
    steam: { ...(current.steam ?? {}), ...(patch.steam ?? {}) },
    steamgrid_db: { ...(current.steamgrid_db ?? {}), ...(patch.steamgrid_db ?? {}) },
    blacklisted_games: patch.blacklisted_games ? [...patch.blacklisted_games] : current.blacklisted_games,
  };
};

const withItem = <T,>(set: Set<T>, item: T, present: boolean): Set<T> => {
  const next = new Set(set);
  if (present) next.add(item);
  else next.delete(item);
  return next;
};

const App = () => {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [platforms, setPlatforms] = useState<PlatformSummary[]>([]);
  const [plan, setPlan] = useState<SyncPlan | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [rescanning, setRescanning] = useState(false);

  const [view, setView] = useState<View>("games");
  const [filter, setFilter] = useState("all");
  const [focusPlatform, setFocusPlatform] = useState<string | null>(null);

  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<SyncOutcome | null>(null);
  const [progress, setProgress] = useState<SyncProgressEvent>({ state: "not_started" });

  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [platformBusy, setPlatformBusy] = useState<Set<string>>(() => new Set());
  const [platformGroups, setPlatformGroups] = useState<PlatformSettingsGroup[]>([]);
  const [platformLoading, setPlatformLoading] = useState(false);
  const [platformError, setPlatformError] = useState<string | null>(null);
  const [platformSaving, setPlatformSaving] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    listen<SyncProgressEvent>("sync-progress", (event) => {
      if (active) setProgress(event.payload);
    })
      .then((stop) => {
        unlisten = stop;
      })
      .catch((err) => console.error("Failed to listen for sync progress", err));
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const fetchAll = useCallback(async () => {
    const [s, p, pl] = await Promise.all([
      invoke<Settings>("load_settings"),
      invoke<PlatformSummary[]>("discover_games"),
      invoke<SyncPlan>("plan_sync"),
    ]);
    setSettings(s);
    setPlatforms(p);
    setPlan(pl);
  }, []);

  const fetchPlatformSettings = useCallback(async () => {
    setPlatformError(null);
    setPlatformLoading(true);
    try {
      const payload = await invoke<PlatformSettingsPayload[]>("list_platform_settings");
      setPlatformGroups(payload.map(buildPlatformSettingsGroup));
    } catch (err) {
      setPlatformError(errorMessage(err));
    } finally {
      setPlatformLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAll()
      .catch((err) => setLoadError(errorMessage(err)))
      .finally(() => setLoading(false));
  }, [fetchAll]);

  useEffect(() => {
    if (view === "settings") void fetchPlatformSettings();
    else setFocusPlatform(null);
  }, [view, fetchPlatformSettings]);

  const rescan = useCallback(async () => {
    setRescanning(true);
    setLoadError(null);
    try {
      await fetchAll();
      if (view === "settings") await fetchPlatformSettings();
    } catch (err) {
      setLoadError(errorMessage(err));
    } finally {
      setRescanning(false);
    }
  }, [fetchAll, fetchPlatformSettings, view]);

  const updateSettings = useCallback(
    async (patch: SettingsUpdatePayload) => {
      setSettingsError(null);
      setSettings((prev) => applySettingsPatch(prev, patch));
      try {
        setSettings(await invoke<Settings>("update_settings", { update: patch }));
        if (patch.blacklisted_games) setPlan(await invoke<SyncPlan>("plan_sync"));
      } catch (err) {
        setSettingsError(errorMessage(err));
        await fetchAll().catch(() => undefined);
      }
    },
    [fetchAll]
  );

  // A game is selected unless it is on the blacklist.
  const blacklist = useMemo(() => new Set(settings?.blacklisted_games ?? []), [settings]);
  const isSelected = useCallback((appId: number) => !blacklist.has(appId), [blacklist]);

  const setSelected = useCallback(
    (appIds: number[], selected: boolean) => {
      const next = new Set(blacklist);
      appIds.forEach((id) => (selected ? next.delete(id) : next.add(id)));
      void updateSettings({ blacklisted_games: [...next] });
    },
    [blacklist, updateSettings]
  );

  const runImport = useCallback(async () => {
    setSyncing(true);
    setSyncError(null);
    setOutcome(null);
    setProgress({ state: "starting" });
    try {
      setOutcome(await invoke<SyncOutcome>("run_full_sync"));
      await fetchAll();
    } catch (err) {
      setSyncError(errorMessage(err));
    } finally {
      setSyncing(false);
    }
  }, [fetchAll]);

  const togglePlatform = useCallback(
    async (codeName: string, enabled: boolean) => {
      const previous = platforms;
      setPlatforms((prev) => prev.map((p) => (p.code_name === codeName ? { ...p, enabled } : p)));
      setPlatformBusy((prev) => withItem(prev, codeName, true));
      try {
        const response = await invoke<PlatformToggleResponse>("update_platform_enabled", { codeName, enabled });
        setPlatforms(response.platforms);
        setPlan(response.plan);
      } catch (err) {
        setLoadError(errorMessage(err));
        setPlatforms(previous);
      } finally {
        setPlatformBusy((prev) => withItem(prev, codeName, false));
      }
    },
    [platforms]
  );

  const openPlatformSettings = useCallback((codeName: string) => {
    setFocusPlatform(codeName);
    setView("settings");
  }, []);

  const changePlatformField = useCallback((codeName: string, update: PlatformFieldUpdate) => {
    setPlatformGroups((prev) =>
      prev.map((group) =>
        group.codeName !== codeName
          ? group
          : {
              ...group,
              fields: group.fields.map((field) =>
                field.key === update.key && field.kind === update.kind
                  ? ({ ...field, value: update.value } as typeof field)
                  : field
              ),
            }
      )
    );
  }, []);

  const resetPlatform = useCallback((codeName: string) => {
    setPlatformGroups((prev) => prev.map((g) => (g.codeName === codeName ? resetGroup(g) : g)));
  }, []);

  const savePlatform = useCallback(
    async (codeName: string) => {
      const group = platformGroups.find((g) => g.codeName === codeName);
      if (!group) return;
      setPlatformError(null);
      setPlatformSaving((prev) => withItem(prev, codeName, true));
      try {
        const payload = await invoke<PlatformSettingsPayload>("update_platform_settings", {
          codeName,
          settings: buildPlatformSettingsPayload(group),
        });
        const merged = mergeOptionalFieldKinds(buildPlatformSettingsGroup(payload), group);
        setPlatformGroups((prev) => prev.map((g) => (g.codeName === codeName ? merged : g)));
        await fetchAll();
      } catch (err) {
        setPlatformError(errorMessage(err));
      } finally {
        setPlatformSaving((prev) => withItem(prev, codeName, false));
      }
    },
    [fetchAll, platformGroups]
  );

  const navStatus = useNavigation({
    onBack: () => setView("games"),
    onPrevView: () => setView("games"),
    onNextView: () => setView("settings"),
    // Y only moves to the Import button; A confirms, so a stray press never imports.
    onImport: () => document.getElementById("import-button")?.focus(),
  });

  const plannedAppIds = useMemo(() => new Set(plan?.additions.map((a) => a.shortcut.app_id) ?? []), [plan]);
  const selectedIn = useCallback((p: PlatformSummary) => p.games.filter((g) => isSelected(g.app_id)).length, [isSelected]);

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <img src={logo} alt="" className="pixelated simmer h-24 w-24" />
        <p className="font-pixel text-lg text-peach">Looking for your games…</p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-4 border-b-3 border-harbour bg-deep px-5 py-2">
        <img src={logo} alt="" className="pixelated h-9 w-9" />
        <span className="font-pixel text-2xl text-foam">BoilR</span>
        <nav aria-label="Main" className="ml-6 flex gap-1">
          {(["games", "settings"] as const).map((v) => (
            <button
              key={v}
              type="button"
              onClick={() => setView(v)}
              aria-current={view === v ? "page" : undefined}
              className={clsx(
                "border-b-3 px-3 py-1.5 font-pixel text-lg transition-colors",
                view === v ? "border-flame text-foam" : "border-transparent text-mauve hover:text-peach"
              )}
            >
              {v === "games" ? "Games" : "Settings"}
            </button>
          ))}
        </nav>
        <span className="ml-auto text-sm text-mauve" title={`Built ${__BUILD_TIME__}`}>
          Preview build {__BUILD_COMMIT__}, {new Date(__BUILD_TIME__).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}
        </span>
        <button type="button" className="link-btn" onClick={rescan} disabled={rescanning || syncing}>
          {rescanning ? "Looking…" : "Look for games again"}
        </button>
      </header>

      {loadError ? (
        <p role="alert" className="border-b-2 border-ember bg-harbour/60 px-5 py-2 text-peach">
          Something went wrong: {loadError}
        </p>
      ) : null}

      <main className="min-h-0 flex-1">
        {view === "games" ? (
          <div className="grid h-full grid-cols-[15rem_1fr]">
            <aside className="min-h-0 border-r-3 border-harbour bg-deep/50">
              <Sources platforms={platforms} selectedIds={selectedIn} filter={filter} onFilter={setFilter} />
            </aside>
            <div className="min-h-0">
              <GameList
                platforms={platforms}
                filter={filter}
                isSelected={isSelected}
                plannedAppIds={plannedAppIds}
                onToggleGame={(id, v) => setSelected([id], v)}
                onSetMany={setSelected}
                onTogglePlatform={togglePlatform}
                busyPlatforms={platformBusy}
                onOpenSettings={openPlatformSettings}
              />
            </div>
          </div>
        ) : (
          <SettingsView
            settings={settings}
            onUpdate={updateSettings}
            error={settingsError}
            platformGroups={platformGroups}
            platformLoading={platformLoading}
            platformError={platformError}
            platformSaving={platformSaving}
            onFieldChange={changePlatformField}
            onReset={resetPlatform}
            onSavePlatform={savePlatform}
            focusPlatform={focusPlatform}
            navStatus={navStatus}
          />
        )}
      </main>

      <PipeBar
        toAdd={plan?.additions.length ?? 0}
        removals={plan?.removals ?? []}
        syncing={syncing}
        progress={progress}
        outcome={outcome}
        syncError={syncError}
        restartsSteam={Boolean(settings?.steam?.start_steam)}
        hasGames={platforms.some((p) => p.enabled && p.games.length > 0)}
        padConnected={navStatus.pads.length > 0}
        onImport={runImport}
      />
    </div>
  );
};

export default App;
