import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import clsx from "clsx";

import type {
  AdditionPlan,
  PlatformSummary,
  PlatformToggleResponse,
  PlatformSettingsPayload,
  RemovalPlan,
  SettingsUpdatePayload,
  SyncOutcome,
  SyncPlan,
  SyncProgressEvent,
} from "./types";
import type { Settings } from "./settings";

const FALLBACK_ART_GRADIENT =
  "bg-[linear-gradient(135deg,#434dff33,#5b6bff11)] border border-white/5";

const errorMessage = (err: unknown): string => {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === "string") {
    return err;
  }
  return "Unknown error";
};

const toImageSrc = (icon?: string | null): string | null => {
  if (!icon) {
    return null;
  }
  try {
    return convertFileSrc(icon);
  } catch (err) {
    console.warn("Failed to resolve icon", icon, err);
    return null;
  }
};

const formatShortcutPath = (exe: string): string => {
  if (!exe) {
    return "";
  }
  if (exe.length <= 48) {
    return exe;
  }
  return `${exe.slice(0, 22)}…${exe.slice(-22)}`;
};

const applySettingsPatch = (
  current: Settings | null,
  patch: SettingsUpdatePayload
): Settings | null => {
  if (!current) {
    return current;
  }

  const next: Settings = {
    ...current,
    steam: { ...(current.steam ?? {}) },
    steamgrid_db: { ...(current.steamgrid_db ?? {}) },
  };

  if (patch.steam) {
    next.steam = { ...(next.steam ?? {}), ...patch.steam };
  }

  if (patch.steamgrid_db) {
    next.steamgrid_db = {
      ...(next.steamgrid_db ?? {}),
      ...patch.steamgrid_db,
    };
  }

  if (patch.blacklisted_games) {
    next.blacklisted_games = [...patch.blacklisted_games];
  }

  return next;
};

type BooleanField = {
  key: string;
  label: string;
  kind: "boolean";
  value: boolean;
  originalValue: boolean;
};

type StringField = {
  key: string;
  label: string;
  kind: "string";
  value: string;
  originalValue: string;
};

type OptionalStringField = {
  key: string;
  label: string;
  kind: "optional-string";
  value: string;
  originalValue: string | null;
};

type StringListField = {
  key: string;
  label: string;
  kind: "string-list";
  value: string[];
  originalValue: string[];
};

type PlatformSettingsField =
  | BooleanField
  | StringField
  | OptionalStringField
  | StringListField;

type PlatformSettingsGroup = {
  codeName: string;
  name: string;
  fields: PlatformSettingsField[];
};

type PlatformFieldUpdate =
  | { key: string; kind: "boolean"; value: boolean }
  | { key: string; kind: "string"; value: string }
  | { key: string; kind: "optional-string"; value: string }
  | { key: string; kind: "string-list"; value: string[] };

const humaniseKey = (raw: string): string =>
  raw
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .trim();

const normaliseStringList = (list: string[]): string[] =>
  list.map((entry) => entry.trim()).filter((entry) => entry.length > 0);

const buildPlatformSettingsGroup = (
  payload: PlatformSettingsPayload
): PlatformSettingsGroup => {
  const entries = Object.entries(payload.settings ?? {}).sort((a, b) => {
    if (a[0] === "enabled") {
      return -1;
    }
    if (b[0] === "enabled") {
      return 1;
    }
    return a[0].localeCompare(b[0]);
  });

  const fields: PlatformSettingsField[] = [];

  for (const [key, raw] of entries) {
    const label = humaniseKey(key);
    if (typeof raw === "boolean") {
      fields.push({ key, label, kind: "boolean", value: raw, originalValue: raw });
      continue;
    }

    if (typeof raw === "string") {
      fields.push({ key, label, kind: "string", value: raw, originalValue: raw });
      continue;
    }

    if (raw === null) {
      fields.push({ key, label, kind: "optional-string", value: "", originalValue: null });
      continue;
    }

    if (Array.isArray(raw) && raw.every((value) => typeof value === "string")) {
      const list = raw as string[];
      fields.push({
        key,
        label,
        kind: "string-list",
        value: [...list],
        originalValue: [...list],
      });
    }
  }

  return {
    codeName: payload.code_name,
    name: payload.name,
    fields,
  };
};

const mergeOptionalFieldKinds = (
  next: PlatformSettingsGroup,
  previous: PlatformSettingsGroup
): PlatformSettingsGroup => {
  const optionalKeys = new Set(
    previous.fields
      .filter((field) => field.kind === "optional-string")
      .map((field) => field.key)
  );

  if (!optionalKeys.size) {
    return next;
  }

  const fields = next.fields.map((field) => {
    if (field.kind === "string" && optionalKeys.has(field.key)) {
      return {
        key: field.key,
        label: field.label,
        kind: "optional-string" as const,
        value: field.value,
        originalValue: field.value,
      } satisfies OptionalStringField;
    }
    return field;
  });

  return { ...next, fields };
};

const fieldHasChanges = (field: PlatformSettingsField): boolean => {
  switch (field.kind) {
    case "boolean":
      return field.value !== field.originalValue;
    case "string":
      return field.value !== field.originalValue;
    case "optional-string":
      return field.value !== (field.originalValue ?? "");
    case "string-list":
      if (field.value.length !== field.originalValue.length) {
        return true;
      }
      return field.value.some((entry, index) => entry !== field.originalValue[index]);
    default:
      return false;
  }
};

const groupHasChanges = (group: PlatformSettingsGroup): boolean =>
  group.fields.some((field) => fieldHasChanges(field));

const resetGroup = (group: PlatformSettingsGroup): PlatformSettingsGroup => {
  const fields = group.fields.map((field) => {
    switch (field.kind) {
      case "boolean":
        return { ...field, value: field.originalValue };
      case "string":
        return { ...field, value: field.originalValue };
      case "optional-string":
        return { ...field, value: field.originalValue ?? "" };
      case "string-list":
        return { ...field, value: [...field.originalValue] };
      default:
        return field;
    }
  });

  return { ...group, fields };
};

const buildPlatformSettingsPayload = (
  group: PlatformSettingsGroup
): Record<string, unknown> => {
  const payload: Record<string, unknown> = {};
  group.fields.forEach((field) => {
    switch (field.kind) {
      case "boolean":
        payload[field.key] = field.value;
        break;
      case "string":
        payload[field.key] = field.value;
        break;
      case "optional-string":
        payload[field.key] = field.value.trim().length === 0 ? null : field.value;
        break;
      case "string-list":
        payload[field.key] = normaliseStringList(field.value);
        break;
      default:
        break;
    }
  });
  return payload;
};

const badgePalette = {
  planned: "text-emerald-300 bg-emerald-300/10 border border-emerald-400/30",
  warning: "text-amber-300 bg-amber-300/10 border border-amber-400/30",
  danger: "text-rose-300 bg-rose-300/10 border border-rose-400/30",
  neutral: "text-slate-200 bg-slate-200/10 border border-slate-400/20",
};

const SectionCard = ({
  children,
  title,
  description,
}: {
  children: ReactNode;
  title: string;
  description?: string;
}) => (
  <section className="rounded-2xl border border-slate-800/60 bg-slate-900/60 shadow-xl shadow-black/30">
    <header className="border-b border-slate-800/70 px-6 py-4">
      <h2 className="text-lg font-semibold tracking-wide text-white">{title}</h2>
      {description ? (
        <p className="mt-1 text-sm text-slate-400">{description}</p>
      ) : null}
    </header>
    <div className="p-6">{children}</div>
  </section>
);

const SummaryStat = ({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: "emerald" | "amber" | "rose" | "slate";
}) => {
  const accentClass = {
    emerald: "bg-emerald-500/20 text-emerald-200",
    amber: "bg-amber-500/20 text-amber-200",
    rose: "bg-rose-500/20 text-rose-200",
    slate: "bg-slate-500/20 text-slate-200",
  }[accent ?? "slate"];
  return (
    <div className="flex flex-col gap-2 rounded-xl border border-white/5 bg-white/5 p-4 shadow-inner shadow-black/30">
      <span className="text-xs uppercase tracking-widest text-slate-400">
        {label}
      </span>
      <span className={clsx("text-2xl font-semibold", accentClass)}>{value}</span>
    </div>
  );
};

const GameTile = ({
  name,
  subtitle,
  icon,
  badges,
  muted,
  highlight,
}: {
  name: string;
  subtitle?: string;
  icon?: string | null;
  badges?: string[];
  muted?: boolean;
  highlight?: "planned" | "blacklisted" | null;
}) => (
  <article
    className={clsx(
      "flex min-h-[88px] items-center gap-3 rounded-xl border border-slate-800/60 bg-slate-900/70 p-3",
      muted && "opacity-60",
      highlight === "planned" && "border-emerald-400/40 bg-emerald-500/10",
      highlight === "blacklisted" && "border-rose-400/30 bg-rose-500/10"
    )}
  >
    <div
      className={clsx(
        "relative h-16 w-16 overflow-hidden rounded-lg",
        !icon && FALLBACK_ART_GRADIENT
      )}
    >
      {icon ? (
        <img
          src={icon}
          alt={name}
          className="h-full w-full object-cover"
          referrerPolicy="no-referrer"
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-lg font-semibold text-slate-300">
          {name.slice(0, 2).toUpperCase()}
        </div>
      )}
    </div>
    <div className="flex-1 space-y-1">
      <div className="flex items-start gap-2">
        <h4 className="text-base font-semibold text-white">{name}</h4>
      </div>
      {subtitle ? <p className="text-xs text-slate-400">{subtitle}</p> : null}
      {badges && badges.length ? (
        <div className="flex flex-wrap gap-2">
          {badges.map((badge) => (
            <span
              key={badge}
              className="rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[11px] uppercase tracking-widest text-slate-300"
            >
              {badge}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  </article>
);

const RemovalRow = ({ removal }: { removal: RemovalPlan }) => {
  const icon = toImageSrc(removal.shortcut.icon ?? null);
  const reasonLabel =
    removal.reason === "legacy_boilr" ? "Legacy BoilR shortcut" : "Existing duplicate";
  const reasonBadge =
    removal.reason === "legacy_boilr" ? badgePalette.neutral : badgePalette.warning;

  return (
    <li className="flex flex-col gap-3 rounded-xl border border-slate-800/60 bg-slate-900/70 p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex items-center gap-3">
        <div
          className={clsx(
            "h-12 w-12 overflow-hidden rounded-md",
            !icon && FALLBACK_ART_GRADIENT
          )}
        >
          {icon ? (
            <img
              src={icon}
              alt={removal.shortcut.display_name}
              className="h-full w-full object-cover"
              referrerPolicy="no-referrer"
            />
          ) : (
            <div className="flex h-full w-full items-center justify-center text-sm font-semibold text-slate-300">
              {removal.shortcut.display_name.slice(0, 2).toUpperCase()}
            </div>
          )}
        </div>
        <div>
          <p className="text-sm font-semibold text-white">
            {removal.shortcut.display_name}
          </p>
          <p className="text-xs text-slate-400">
            User {removal.user_id} · {formatShortcutPath(removal.shortcut.exe)}
          </p>
        </div>
      </div>
      <span className={clsx("w-fit rounded-full px-3 py-1 text-xs uppercase", reasonBadge)}>
        {reasonLabel}
      </span>
    </li>
  );
};

const ToggleRow = ({
  label,
  description,
  checked,
  disabled,
  onToggle,
}: {
  label: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  onToggle: (value: boolean) => void;
}) => (
  <div className="flex items-center justify-between rounded-lg bg-slate-900/80 px-4 py-3">
    <div className="pe-4">
      <p className="text-sm font-semibold text-white">{label}</p>
      {description ? (
        <p className="text-xs text-slate-400">{description}</p>
      ) : null}
    </div>
    <label className="relative inline-flex cursor-pointer items-center">
      <input
        type="checkbox"
        className="peer sr-only"
        checked={checked}
        onChange={(event) => onToggle(event.target.checked)}
        disabled={disabled}
      />
      <span className="h-6 w-11 rounded-full bg-slate-600 transition peer-checked:bg-emerald-500 peer-disabled:bg-slate-700" />
      <span className="absolute left-1 top-1 h-4 w-4 rounded-full bg-white shadow transition peer-checked:translate-x-5 peer-disabled:bg-slate-300" />
    </label>
  </div>
);

const SettingsPanel = ({
  settings,
  saving,
  error,
  onUpdate,
  highlightedPlatform,
  highlightedPlatformCode,
  platformSettings,
  platformSettingsLoading,
  platformSettingsError,
  platformSettingsSaving,
  onPlatformFieldChange,
  onPlatformReset,
  onPlatformSave,
}: {
  settings: Settings | null;
  saving: boolean;
  error: string | null;
  onUpdate: (patch: SettingsUpdatePayload) => void;
  highlightedPlatform?: string | null;
  highlightedPlatformCode?: string | null;
  platformSettings: PlatformSettingsGroup[];
  platformSettingsLoading: boolean;
  platformSettingsError: string | null;
  platformSettingsSaving: Set<string>;
  onPlatformFieldChange: (codeName: string, update: PlatformFieldUpdate) => void;
  onPlatformReset: (codeName: string) => void;
  onPlatformSave: (codeName: string) => void;
}) => {
  const steam = settings?.steam ?? {};
  const grid = settings?.steamgrid_db ?? {};
  const [locationDraft, setLocationDraft] = useState<string>(
    steam.location ?? ""
  );
  const [apiKeyDraft, setApiKeyDraft] = useState<string>(
    grid.auth_key ?? ""
  );

  useEffect(() => {
    setLocationDraft(steam.location ?? "");
  }, [steam.location]);

  useEffect(() => {
    setApiKeyDraft(grid.auth_key ?? "");
  }, [grid.auth_key]);

  if (!settings) {
    return (
      <div className="space-y-6">
        <SectionCard
          title="Sync Preferences"
          description="Key options from your current BoilR settings"
        >
          <p className="text-sm text-slate-400">Settings are unavailable.</p>
        </SectionCard>
      </div>
    );
  }

  const rows: Array<{
    key: string;
    label: string;
    description?: string;
    checked: boolean;
    onToggle: (value: boolean) => void;
  }> = [
    {
      key: "stop_steam",
      label: "Stop Steam before syncing",
      description: "Ensures Steam is closed before BoilR touches shortcut files",
      checked: Boolean(steam.stop_steam),
      onToggle: (value) => onUpdate({ steam: { stop_steam: value } }),
    },
    {
      key: "start_steam",
      label: "Restart Steam after sync",
      description: "Launch Steam again once imports finish",
      checked: Boolean(steam.start_steam),
      onToggle: (value) => onUpdate({ steam: { start_steam: value } }),
    },
    {
      key: "collections",
      label: "Create Steam collections",
      description: "Group imported shortcuts by platform",
      checked: Boolean(steam.create_collections),
      onToggle: (value) => onUpdate({ steam: { create_collections: value } }),
    },
    {
      key: "big_picture",
      label: "Optimise icons for Big Picture",
      description: "Choose image variants sized for Big Picture / Steam Deck",
      checked: Boolean(steam.optimize_for_big_picture),
      onToggle: (value) => onUpdate({ steam: { optimize_for_big_picture: value } }),
    },
    {
      key: "sgdb_enabled",
      label: "Download artwork from SteamGridDB",
      description: "Fetch cover art after shortcuts are created",
      checked: Boolean(grid.enabled),
      onToggle: (value) => onUpdate({ steamgrid_db: { enabled: value } }),
    },
    {
      key: "sgdb_animated",
      label: "Prefer animated artwork",
      description: "Use animated covers when available",
      checked: Boolean(grid.prefer_animated),
      onToggle: (value) => onUpdate({ steamgrid_db: { prefer_animated: value } }),
    },
    {
      key: "sgdb_nsfw",
      label: "Allow NSFW artwork",
      description: "Permit SteamGridDB to return mature-rated art",
      checked: Boolean(grid.allow_nsfw),
      onToggle: (value) => onUpdate({ steamgrid_db: { allow_nsfw: value } }),
    },
    {
      key: "sgdb_only_boilr",
      label: "Only download art for BoilR shortcuts",
      description: "Skip artwork updates for non-BoilR entries",
      checked: Boolean(grid.only_download_boilr_images),
      onToggle: (value) =>
        onUpdate({ steamgrid_db: { only_download_boilr_images: value } }),
    },
  ];

  const groupRefs = useRef<Record<string, HTMLDivElement | null>>({});

  useEffect(() => {
    if (!highlightedPlatformCode || platformSettingsLoading) {
      return;
    }
    const node = groupRefs.current[highlightedPlatformCode];
    if (node) {
      node.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, [highlightedPlatformCode, platformSettings, platformSettingsLoading]);

  return (
    <div className="space-y-6">
      {highlightedPlatform ? (
        <div className="rounded-2xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-100 shadow-inner shadow-emerald-900/30">
          Adjust the settings for{" "}
          <span className="font-semibold text-emerald-50">{highlightedPlatform}</span> below.
        </div>
      ) : null}
      {error ? (
        <div className="rounded-2xl border border-rose-500/40 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
          {error}
        </div>
      ) : null}
      <SectionCard
        title="Steam integration"
        description="Control how BoilR interacts with your Steam installation"
      >
        <div className="space-y-3">
          {rows.slice(0, 4).map((row) => (
            <ToggleRow
              key={row.key}
              label={row.label}
              description={row.description}
              checked={row.checked}
              disabled={saving}
              onToggle={row.onToggle}
            />
          ))}
          <div className="rounded-lg bg-slate-900/80 p-4">
            <label className="flex flex-col gap-2 text-sm text-slate-300">
              Steam install folder
              <input
                className="rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white focus:border-emerald-400 focus:outline-none"
                placeholder="Let BoilR autodetect by leaving blank"
                value={locationDraft}
                onChange={(event) => setLocationDraft(event.target.value)}
                disabled={saving}
              />
            </label>
            <div className="mt-3 flex gap-2">
              <button
                type="button"
                className="rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-semibold text-slate-200 hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={() => setLocationDraft("")}
                disabled={saving}
              >
                Reset
              </button>
              <button
                type="button"
                className="rounded-lg bg-emerald-500/90 px-3 py-1.5 text-xs font-semibold text-emerald-50 hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={() =>
                  onUpdate({
                    steam: { location: locationDraft.trim() ? locationDraft.trim() : null },
                  })
                }
                disabled={saving}
              >
                Save location
              </button>
            </div>
          </div>
        </div>
      </SectionCard>

      <SectionCard
        title="Artwork"
        description="Manage how SteamGridDB assets are downloaded and applied"
      >
        <div className="space-y-4">
          <div className="rounded-lg bg-slate-900/80 p-4">
            <label className="flex flex-col gap-2 text-sm text-slate-300">
              SteamGridDB API key
              <input
                className="rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white focus:border-emerald-400 focus:outline-none"
                placeholder="Paste your SteamGridDB API key"
                value={apiKeyDraft}
                onChange={(event) => setApiKeyDraft(event.target.value)}
                disabled={saving}
              />
            </label>
            <p className="mt-2 text-xs text-slate-400">
              {grid.auth_key
                ? `Current key: ${grid.auth_key.slice(0, 4)}…${grid.auth_key.slice(-4)}`
                : "No key stored yet."}
            </p>
            <div className="mt-3 flex gap-2">
              <button
                type="button"
                className="rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-semibold text-slate-200 hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={() => setApiKeyDraft("")}
                disabled={saving}
              >
                Clear
              </button>
              <button
                type="button"
                className="rounded-lg bg-emerald-500/90 px-3 py-1.5 text-xs font-semibold text-emerald-50 hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={() =>
                  onUpdate({
                    steamgrid_db: {
                      auth_key: apiKeyDraft.trim() ? apiKeyDraft.trim() : null,
                    },
                  })
                }
                disabled={saving}
              >
                Save API key
              </button>
            </div>
          </div>

          {rows.slice(4).map((row) => (
            <ToggleRow
              key={row.key}
              label={row.label}
              description={row.description}
              checked={row.checked}
              disabled={saving}
              onToggle={row.onToggle}
            />
          ))}
        </div>
      </SectionCard>

      <SectionCard
        title="Platform settings"
        description="Tweak launcher-specific options such as paths and runtime behaviour"
      >
        <div className="space-y-4">
          {platformSettingsError ? (
            <div className="rounded-xl border border-rose-500/40 bg-rose-500/10 px-3 py-2 text-sm text-rose-100">
              {platformSettingsError}
            </div>
          ) : null}
          {platformSettingsLoading ? (
            <div className="flex items-center justify-center gap-2 text-sm text-slate-300">
              <span className="h-3 w-3 animate-spin rounded-full border border-emerald-200 border-t-transparent" />
              Loading platform settings…
            </div>
          ) : platformSettings.length ? (
            platformSettings.map((group) => {
              const savingGroup = platformSettingsSaving.has(group.codeName);
              const hasChanges = groupHasChanges(group);
              const disabled = saving || savingGroup;
              const isHighlighted = highlightedPlatform === group.name;

              return (
                <div
                  key={group.codeName}
                  className={clsx(
                    "space-y-4 rounded-2xl border border-slate-800/60 bg-slate-900/70 p-4",
                    isHighlighted && "border-emerald-400/50 shadow-lg shadow-emerald-500/10"
                  )}
                  ref={(element) => {
                    groupRefs.current[group.codeName] = element;
                  }}
                >
                  <header className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div>
                      <h3 className="text-base font-semibold text-white">{group.name}</h3>
                      <p className="text-xs uppercase tracking-widest text-slate-500">
                        {group.codeName}
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => onPlatformReset(group.codeName)}
                        disabled={!hasChanges || disabled}
                        className="rounded-full border border-slate-700/70 bg-slate-900/70 px-3 py-1 text-xs font-semibold text-slate-300 transition hover:border-slate-500/70 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        Reset
                      </button>
                      <button
                        type="button"
                        onClick={() => onPlatformSave(group.codeName)}
                        disabled={!hasChanges || disabled}
                        className="inline-flex items-center gap-2 rounded-full bg-emerald-500/90 px-3 py-1 text-xs font-semibold text-emerald-50 shadow-lg shadow-emerald-500/20 transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {savingGroup ? (
                          <span className="h-3 w-3 animate-spin rounded-full border border-emerald-100 border-t-transparent" />
                        ) : null}
                        Save
                      </button>
                    </div>
                  </header>

                  {group.fields.length ? (
                    <div className="space-y-3">
                      {group.fields.map((field) => {
                        switch (field.kind) {
                          case "boolean":
                            return (
                              <ToggleRow
                                key={field.key}
                                label={field.label}
                                checked={field.value}
                                disabled={disabled}
                                onToggle={(value) =>
                                  onPlatformFieldChange(group.codeName, {
                                    key: field.key,
                                    kind: "boolean",
                                    value,
                                  })
                                }
                              />
                            );
                          case "string":
                          case "optional-string":
                            return (
                              <div
                                key={field.key}
                                className="space-y-2 rounded-lg bg-slate-900/80 p-4"
                              >
                                <label className="block text-sm font-semibold text-white">
                                  {field.label}
                                </label>
                                <input
                                  className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white focus:border-emerald-400 focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
                                  value={field.value}
                                  placeholder={
                                    field.kind === "optional-string"
                                      ? "Leave blank to clear this value"
                                      : undefined
                                  }
                                  disabled={disabled}
                                  onChange={(event) =>
                                    onPlatformFieldChange(group.codeName, {
                                      key: field.key,
                                      kind: field.kind,
                                      value: event.target.value,
                                    })
                                  }
                                />
                              </div>
                            );
                          case "string-list":
                            return (
                              <div
                                key={field.key}
                                className="space-y-2 rounded-lg bg-slate-900/80 p-4"
                              >
                                <label className="block text-sm font-semibold text-white">
                                  {field.label}
                                </label>
                                <textarea
                                  className="h-28 w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white focus:border-emerald-400 focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
                                  value={field.value.join("\n")}
                                  disabled={disabled}
                                  onChange={(event) =>
                                    onPlatformFieldChange(group.codeName, {
                                      key: field.key,
                                      kind: "string-list",
                                      value: event.target.value.split(/\r?\n/),
                                    })
                                  }
                                />
                                <p className="text-xs text-slate-400">One entry per line.</p>
                              </div>
                            );
                          default:
                            return null;
                        }
                      })}
                    </div>
                  ) : (
                    <p className="text-sm text-slate-400">No configurable options available.</p>
                  )}
                </div>
              );
            })
          ) : (
            <p className="text-sm text-slate-400">No platform settings detected.</p>
          )}
        </div>
      </SectionCard>

      <SectionCard
        title="Library hygiene"
        description="At-a-glance counters for advanced library rules"
      >
        <dl className="grid gap-4 sm:grid-cols-2">
          <div className="rounded-lg bg-slate-900/80 p-3">
            <dt className="text-xs uppercase tracking-widest text-slate-400">
              Blacklisted games
            </dt>
            <dd className="text-lg font-semibold text-white">
              {settings.blacklisted_games?.length ?? 0}
            </dd>
          </div>
          <div className="rounded-lg bg-slate-900/80 p-3">
            <dt className="text-xs uppercase tracking-widest text-slate-400">
              Steam location override
            </dt>
            <dd className="text-sm font-medium text-white">
              {steam.location ? steam.location : "Auto-detected"}
            </dd>
          </div>
        </dl>
        {saving ? (
          <p className="mt-3 text-xs text-slate-400">Saving changes…</p>
        ) : null}
      </SectionCard>
    </div>
  );
};

const PlanSummaryCard = ({
  additions,
  removals,
  unchanged,
  refreshing,
}: {
  additions: number;
  removals: number;
  unchanged: number;
  refreshing: boolean;
}) => (
  <SectionCard
    title="Upcoming changes"
    description={
      refreshing ? "Refreshing plan…" : "Preview of the library changes before the next sync"
    }
  >
    <div className="grid gap-4 sm:grid-cols-3">
      <SummaryStat label="Shortcuts to add" value={`${additions}`} accent="emerald" />
      <SummaryStat label="Shortcuts to remove" value={`${removals}`} accent="rose" />
      <SummaryStat label="Unchanged shortcuts" value={`${Math.max(unchanged, 0)}`} accent="slate" />
    </div>
  </SectionCard>
);

const PlatformLibrary = ({
  platforms,
  plannedAppIds,
  additionsByPlatform,
  onTogglePlatform,
  busyPlatforms,
  onRefreshPlatform,
  onOpenPlatformSettings,
  refreshing,
  syncRunning,
}: {
  platforms: PlatformSummary[];
  plannedAppIds: Set<number>;
  additionsByPlatform: Map<string, AdditionPlanGroup>;
  onTogglePlatform: (codeName: string, enabled: boolean) => void;
  busyPlatforms: Set<string>;
  onRefreshPlatform: (codeName: string) => void;
  onOpenPlatformSettings: (platform: PlatformSummary) => void;
  refreshing: boolean;
  syncRunning: boolean;
}) => (
  <SectionCard
    title="Discovered platforms"
    description="Review games found across enabled launchers and how they will be treated"
  >
    <div className="grid gap-4 lg:grid-cols-2">
      {platforms.map((platform) => {
        const plannedGroup = additionsByPlatform.get(platform.code_name);
        const isBusy = busyPlatforms.has(platform.code_name);
        const statusClasses = platform.enabled
          ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
          : "border-slate-700/50 bg-slate-800/80 text-slate-300";
        return (
          <div
            key={platform.code_name}
            className="flex flex-col gap-4 rounded-2xl border border-slate-800/70 bg-slate-900/80 p-4"
          >
            <header className="flex flex-col gap-3 border-b border-slate-800/60 pb-3">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-2">
                  <h3 className="text-lg font-semibold text-white">{platform.name}</h3>
                  <span
                    className={clsx(
                      "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[11px] uppercase tracking-widest",
                      statusClasses
                    )}
                  >
                    {platform.enabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <label className="relative inline-flex cursor-pointer items-center">
                    <input
                      type="checkbox"
                      className="peer sr-only"
                      aria-label={`Toggle ${platform.name}`}
                      checked={platform.enabled}
                      disabled={isBusy}
                      onChange={(event) =>
                        onTogglePlatform(platform.code_name, event.target.checked)
                      }
                    />
                    <span className="h-6 w-11 rounded-full bg-slate-700 transition peer-checked:bg-emerald-500 peer-disabled:bg-slate-800" />
                    <span className="absolute left-1 top-1 h-4 w-4 rounded-full bg-white shadow transition peer-checked:translate-x-5 peer-disabled:bg-slate-300" />
                  </label>
                  {isBusy ? (
                    <span className="h-3 w-3 animate-spin rounded-full border border-emerald-200 border-t-transparent" />
                  ) : null}
                </div>
              </div>
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex items-center gap-2">
                  <span className="rounded-full bg-slate-800/80 px-3 py-1 text-xs text-slate-300">
                    {platform.games.length} discovered
                  </span>
                  {plannedGroup ? (
                    <span className="rounded-full bg-emerald-500/10 px-3 py-1 text-xs text-emerald-300">
                      {plannedGroup.entries.length} queued
                    </span>
                  ) : null}
                </div>
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => onRefreshPlatform(platform.code_name)}
                    disabled={refreshing || syncRunning || isBusy}
                    className="inline-flex items-center gap-2 rounded-full border border-slate-700/70 bg-slate-900/70 px-3 py-1 text-xs font-semibold text-slate-300 transition hover:border-slate-500/70 hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    Refresh now
                  </button>
                  <button
                    type="button"
                    onClick={() => onOpenPlatformSettings(platform)}
                    className="inline-flex items-center gap-2 rounded-full border border-emerald-400/40 bg-emerald-500/10 px-3 py-1 text-xs font-semibold text-emerald-200 transition hover:border-emerald-400/60 hover:text-emerald-100"
                  >
                    Open platform settings
                  </button>
                </div>
              </div>
            </header>

            <div className="grid gap-3">
              {platform.games.length ? (
                platform.games.map((game) => {
                  const planned = plannedAppIds.has(game.app_id);
                  const highlight = game.blacklisted
                    ? "blacklisted"
                    : planned
                    ? "planned"
                    : null;
                  const icon = toImageSrc(game.icon ?? null);
                  const badges = [] as string[];
                  if (planned) {
                    badges.push("Queued for import");
                  }
                  if (game.blacklisted) {
                    badges.push("Blacklisted");
                  }
                  if (game.needs_proton) {
                    badges.push("Proton");
                  }
                  if (game.needs_symlinks) {
                    badges.push("Symlinks");
                  }
                  return (
                    <GameTile
                      key={game.app_id}
                      name={game.display_name}
                      subtitle={formatShortcutPath(game.exe)}
                      icon={icon}
                      badges={badges}
                      muted={!platform.enabled}
                      highlight={highlight}
                    />
                  );
                })
              ) : (
                <p className="text-sm text-slate-400">
                  No games detected for this platform yet.
                </p>
              )}
            </div>

            {platform.error ? (
              <p className="rounded-lg bg-rose-500/10 p-3 text-sm text-rose-200">
                {platform.error}
              </p>
            ) : null}
          </div>
        );
      })}
    </div>
  </SectionCard>
);

type AdditionPlanGroup = {
  platform: string;
  platformCode: string;
  entries: AdditionPlan[];
};

const RemovalList = ({ removals }: { removals: RemovalPlan[] }) => (
  <SectionCard
    title="Shortcuts scheduled for removal"
    description="Existing Steam shortcuts that will be replaced or cleaned up"
  >
    {removals.length ? (
      <ul className="space-y-3">
        {removals.map((removal) => (
          <RemovalRow key={`${removal.user_id}-${removal.shortcut.app_id}-${removal.reason}`} removal={removal} />
        ))}
      </ul>
    ) : (
      <p className="text-sm text-slate-400">
        No existing shortcuts need to be removed based on the current configuration.
      </p>
    )}
  </SectionCard>
);

const ActionsBar = ({
  refreshing,
  onRefresh,
  syncRunning,
  onRunSync,
  syncError,
  lastOutcome,
  progress,
}: {
  refreshing: boolean;
  onRefresh: () => void;
  syncRunning: boolean;
  onRunSync: () => void;
  syncError: string | null;
  lastOutcome: SyncOutcome | null;
  progress: SyncProgressEvent;
}) => {
  const progressMessage = (() => {
    switch (progress.state) {
      case "starting":
        return "Starting import…";
      case "found_games":
        return progress.games_found !== undefined
          ? `Discovered ${progress.games_found} shortcut${progress.games_found === 1 ? "" : "s"} to import…`
          : "Calculating shortcuts…";
      case "finding_images":
        return "Searching for artwork…";
      case "downloading_images":
        return progress.to_download !== undefined
          ? `Downloading ${progress.to_download} image${progress.to_download === 1 ? "" : "s"}…`
          : "Downloading artwork…";
      case "done":
        return "Finalising Steam updates…";
      default:
        return "Preparing sync…";
    }
  })();

  return (
  <SectionCard
    title="Actions"
    description="Kick off a full sync once you’re happy with the plan"
  >
    <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex gap-3">
        <button
          type="button"
          onClick={onRefresh}
          disabled={refreshing || syncRunning}
          className="inline-flex items-center gap-2 rounded-xl border border-slate-700/70 bg-slate-900/70 px-4 py-2 text-sm font-medium text-slate-200 hover:border-slate-500/70 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
        >
          <span className="relative flex items-center gap-2">
            {refreshing ? (
              <span className="h-3 w-3 animate-spin rounded-full border border-slate-400 border-t-transparent" />
            ) : null}
            Refresh plan
          </span>
        </button>
        <button
          type="button"
          onClick={onRunSync}
          disabled={syncRunning || refreshing}
          className="inline-flex items-center gap-2 rounded-xl bg-emerald-500/90 px-4 py-2 text-sm font-semibold text-emerald-50 shadow-lg shadow-emerald-500/20 transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {syncRunning ? (
            <span className="h-3 w-3 animate-spin rounded-full border border-emerald-100 border-t-transparent" />
          ) : null}
          Run import pipeline
        </button>
      </div>
      <div className="text-sm text-slate-400">
        {syncError ? (
          <span className="text-rose-300">Sync failed: {syncError}</span>
        ) : syncRunning ? (
          <span className="inline-flex items-center gap-2 text-emerald-200">
            <span className="h-2.5 w-2.5 animate-pulse rounded-full bg-emerald-400" />
            {progressMessage}
          </span>
        ) : lastOutcome ? (
          <span>
            Last run: imported {lastOutcome.imported_platforms} platform(s),
            updated {lastOutcome.steam_users_updated} Steam user(s)
          </span>
        ) : (
          <span>No sync has been executed in this session.</span>
        )}
      </div>
    </div>
  </SectionCard>
  );
};

const App = () => {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [platforms, setPlatforms] = useState<PlatformSummary[]>([]);
  const [plan, setPlan] = useState<SyncPlan | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [syncRunning, setSyncRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [lastOutcome, setLastOutcome] = useState<SyncOutcome | null>(null);
  const [progress, setProgress] = useState<SyncProgressEvent>({ state: "not_started" });
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [platformBusy, setPlatformBusy] = useState<Set<string>>(() => new Set());
  const [highlightedPlatform, setHighlightedPlatform] = useState<string | null>(null);
  const [highlightedPlatformCode, setHighlightedPlatformCode] = useState<string | null>(null);
  const [activeView, setActiveView] = useState<"overview" | "settings">("overview");
  const [platformSettings, setPlatformSettings] = useState<PlatformSettingsGroup[]>([]);
  const [platformSettingsLoading, setPlatformSettingsLoading] = useState(false);
  const [platformSettingsError, setPlatformSettingsError] = useState<string | null>(null);
  const [platformSettingsSaving, setPlatformSettingsSaving] = useState<Set<string>>(
    () => new Set()
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;

    listen<SyncProgressEvent>("sync-progress", (event) => {
      if (active) {
        setProgress(event.payload);
      }
    })
      .then((stop) => {
        unlisten = stop;
      })
      .catch((err) => console.error("Failed to register sync-progress listener", err));

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const fetchAll = useCallback(async () => {
    setSettingsError(null);
    const [settingsRes, platformsRes, planRes] = await Promise.all([
      invoke<Settings>("load_settings"),
      invoke<PlatformSummary[]>("discover_games"),
      invoke<SyncPlan>("plan_sync"),
    ]);

    setSettings(settingsRes);
    setPlatforms(platformsRes);
    setPlan(planRes);
  }, []);

  const fetchPlatformSettings = useCallback(async () => {
    setPlatformSettingsError(null);
    setPlatformSettingsLoading(true);
    try {
      const payload = await invoke<PlatformSettingsPayload[]>("list_platform_settings");
      setPlatformSettings(payload.map(buildPlatformSettingsGroup));
    } catch (err) {
      const message = errorMessage(err);
      setPlatformSettingsError(message);
    } finally {
      setPlatformSettingsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAll()
      .catch((err) => {
        const message = errorMessage(err);
        console.error("Failed to initialise dashboard", err);
        setError(message);
      })
      .finally(() => setLoading(false));
  }, [fetchAll]);

  useEffect(() => {
    if (activeView !== "settings") {
      setHighlightedPlatform(null);
      setHighlightedPlatformCode(null);
    }
  }, [activeView]);

  useEffect(() => {
    if (activeView === "settings") {
      fetchPlatformSettings();
    }
  }, [activeView, fetchPlatformSettings]);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    try {
      await fetchAll();
      if (activeView === "settings") {
        await fetchPlatformSettings();
      }
    } catch (err) {
      const message = errorMessage(err);
      setError(message);
    } finally {
      setRefreshing(false);
    }
  }, [activeView, fetchAll, fetchPlatformSettings]);

  const handleSettingsUpdate = useCallback(
    async (patch: SettingsUpdatePayload) => {
      setSettingsSaving(true);
      setSettingsError(null);
      setSettings((prev) => applySettingsPatch(prev, patch));

      try {
        const updated = await invoke<Settings>("update_settings", { update: patch });
        setSettings(updated);
      } catch (err) {
        const message = errorMessage(err);
        setSettingsError(message);
        try {
          await fetchAll();
        } catch (refreshErr) {
          console.error("Failed to refresh after settings error", refreshErr);
        }
      } finally {
        setSettingsSaving(false);
      }
    },
    [fetchAll]
  );

  const handleRunSync = useCallback(async () => {
    setSyncRunning(true);
    setSyncError(null);
    setProgress({ state: "starting" });
    try {
      const outcome = await invoke<SyncOutcome>("run_full_sync");
      setLastOutcome(outcome);
      await fetchAll();
    } catch (err) {
      const message = errorMessage(err);
      setSyncError(message);
    } finally {
      setSyncRunning(false);
    }
  }, [fetchAll]);

  const handlePlatformSettingsShortcut = useCallback((platform: PlatformSummary) => {
    setHighlightedPlatform(platform.name);
    setHighlightedPlatformCode(platform.code_name);
    setActiveView("settings");
  }, []);

  const handleRefreshPlatform = useCallback(
    async (_codeName: string) => {
      await handleRefresh();
    },
    [handleRefresh]
  );

  const handlePlatformFieldChange = useCallback(
    (codeName: string, update: PlatformFieldUpdate) => {
      setPlatformSettings((prev) =>
        prev.map((group) => {
          if (group.codeName !== codeName) {
            return group;
          }

          const fields = group.fields.map((field) => {
            if (field.key !== update.key) {
              return field;
            }

            switch (field.kind) {
              case "boolean":
                if (update.kind === "boolean") {
                  return { ...field, value: update.value };
                }
                return field;
              case "string":
                if (update.kind === "string") {
                  return { ...field, value: update.value };
                }
                return field;
              case "optional-string":
                if (update.kind === "optional-string") {
                  return { ...field, value: update.value };
                }
                return field;
              case "string-list":
                if (update.kind === "string-list") {
                  return { ...field, value: [...update.value] };
                }
                return field;
              default:
                return field;
            }
          });

          return { ...group, fields };
        })
      );
    },
    []
  );

  const handleResetPlatformSettings = useCallback((codeName: string) => {
    setPlatformSettings((prev) =>
      prev.map((group) => (group.codeName === codeName ? resetGroup(group) : group))
    );
  }, []);

  const handleSavePlatformSettings = useCallback(
    async (codeName: string) => {
      const group = platformSettings.find((item) => item.codeName === codeName);
      if (!group) {
        return;
      }

      setPlatformSettingsError(null);
      setPlatformSettingsSaving((prev) => {
        const next = new Set(prev);
        next.add(codeName);
        return next;
      });

      try {
        const payload = await invoke<PlatformSettingsPayload>("update_platform_settings", {
          codeName,
          settings: buildPlatformSettingsPayload(group),
        });
        const merged = mergeOptionalFieldKinds(
          buildPlatformSettingsGroup(payload),
          group
        );
        setPlatformSettings((prev) =>
          prev.map((existing) =>
            existing.codeName === codeName
              ? merged
              : existing
          )
        );
        await fetchAll();
        await fetchPlatformSettings();
      } catch (err) {
        const message = errorMessage(err);
        setPlatformSettingsError(message);
      } finally {
        setPlatformSettingsSaving((prev) => {
          const next = new Set(prev);
          next.delete(codeName);
          return next;
        });
      }
    },
    [fetchAll, fetchPlatformSettings, platformSettings]
  );

  const handleTogglePlatform = useCallback(
    async (codeName: string, enabled: boolean) => {
      const previousPlatforms = platforms.map((platform) => ({
        ...platform,
        games: platform.games.map((game) => ({ ...game })),
      }));
      setPlatforms((prev) =>
        prev.map((platform) =>
          platform.code_name === codeName ? { ...platform, enabled } : platform
        )
      );
      setPlatformBusy((prev) => {
        const next = new Set(prev);
        next.add(codeName);
        return next;
      });
      setError(null);
      try {
        const response = await invoke<PlatformToggleResponse>("update_platform_enabled", {
          codeName,
          enabled,
        });
        setPlatforms(response.platforms);
        setPlan(response.plan);
      } catch (err) {
        const message = errorMessage(err);
        setError(message);
        setPlatforms(previousPlatforms);
        try {
          await fetchAll();
        } catch (refreshErr) {
          console.error("Failed to refresh after platform toggle error", refreshErr);
        }
      } finally {
        setPlatformBusy((prev) => {
          const next = new Set(prev);
          next.delete(codeName);
          return next;
        });
      }
    },
    [fetchAll, platforms]
  );

  const additionsCount = plan?.additions.length ?? 0;
  const removalsCount = plan?.removals.length ?? 0;
  const plannedAppIds = useMemo(() => {
    if (!plan) {
      return new Set<number>();
    }
    return new Set(plan.additions.map((addition) => addition.shortcut.app_id));
  }, [plan]);

  const importableCount = useMemo(() => {
    return platforms.reduce((acc, platform) => {
      if (!platform.enabled) {
        return acc;
      }
      const available = platform.games.filter((game) => !game.blacklisted).length;
      return acc + available;
    }, 0);
  }, [platforms]);

  const unchangedCount = importableCount - additionsCount;

  const additionsByPlatform = useMemo(() => {
    const map = new Map<string, AdditionPlanGroup>();
    if (!plan) {
      return map;
    }
    plan.additions.forEach((addition) => {
      const existing = map.get(addition.platform_code);
      if (existing) {
        existing.entries.push(addition);
      } else {
        map.set(addition.platform_code, {
          platform: addition.platform,
          platformCode: addition.platform_code,
          entries: [addition],
        });
      }
    });
    return map;
  }, [plan]);

  if (loading) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-3 bg-slate-950 text-slate-100">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-emerald-300 border-t-transparent" />
        <p className="text-sm text-slate-400">Launching BoilR dashboard…</p>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-slate-950 pb-16">
      <header className="border-b border-slate-900 bg-[linear-gradient(120deg,#1f2937,#111827)] px-6 py-6 shadow-lg shadow-black/30">
        <div className="mx-auto flex max-w-6xl flex-col gap-6">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h1 className="text-2xl font-semibold text-white">BoilR Control Panel</h1>
              <p className="text-sm text-slate-300">
                {activeView === "overview"
                  ? "Preview platform imports, artwork downloads, and Steam cleanup before syncing."
                  : "Tweak how BoilR integrates with Steam and pulls artwork."}
              </p>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={handleRefresh}
                disabled={refreshing || syncRunning}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-700/70 bg-slate-900/70 px-4 py-2 text-sm font-medium text-slate-200 hover:border-slate-500/70 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
              >
                {refreshing ? (
                  <span className="h-3 w-3 animate-spin rounded-full border border-slate-300 border-t-transparent" />
                ) : null}
                Refresh
              </button>
              <button
                type="button"
                onClick={handleRunSync}
                disabled={syncRunning || refreshing}
                className="inline-flex items-center gap-2 rounded-xl bg-emerald-500/90 px-4 py-2 text-sm font-semibold text-emerald-50 shadow-lg shadow-emerald-500/20 transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {syncRunning ? (
                  <span className="h-3 w-3 animate-spin rounded-full border border-emerald-100 border-t-transparent" />
                ) : null}
                Run sync
              </button>
            </div>
          </div>
          <nav className="flex gap-3">
            {[
              { key: "overview", label: "Overview" },
              { key: "settings", label: "Settings" },
            ].map((tab) => (
              <button
                key={tab.key}
                type="button"
                onClick={() => setActiveView(tab.key as "overview" | "settings")}
                className={clsx(
                  "rounded-full px-4 py-2 text-sm font-semibold transition",
                  activeView === tab.key
                    ? "bg-emerald-500/20 text-emerald-200 border border-emerald-400/30"
                    : "bg-slate-900/70 text-slate-300 border border-slate-700/60 hover:text-white"
                )}
              >
                {tab.label}
              </button>
            ))}
          </nav>
        </div>
      </header>

      <main className="mx-auto flex max-w-6xl flex-col gap-6 px-6 py-6">
        {error ? (
          <div className="rounded-xl border border-rose-500/40 bg-rose-500/10 p-4 text-sm text-rose-100">
            {error}
          </div>
        ) : null}

        {activeView === "overview" ? (
          <>
            <PlanSummaryCard
              additions={additionsCount}
              removals={removalsCount}
              unchanged={unchangedCount}
              refreshing={refreshing}
            />

            <ActionsBar
              refreshing={refreshing}
              onRefresh={handleRefresh}
              syncRunning={syncRunning}
              onRunSync={handleRunSync}
              syncError={syncError}
              lastOutcome={lastOutcome}
              progress={progress}
            />

            <PlatformLibrary
              platforms={platforms}
              plannedAppIds={plannedAppIds}
              additionsByPlatform={additionsByPlatform}
              onTogglePlatform={handleTogglePlatform}
              busyPlatforms={platformBusy}
              onRefreshPlatform={handleRefreshPlatform}
              onOpenPlatformSettings={handlePlatformSettingsShortcut}
              refreshing={refreshing}
              syncRunning={syncRunning}
            />

            <RemovalList removals={plan?.removals ?? []} />
          </>
        ) : (
          <SettingsPanel
            settings={settings}
            saving={settingsSaving}
            error={settingsError}
            onUpdate={handleSettingsUpdate}
            highlightedPlatform={highlightedPlatform}
            highlightedPlatformCode={highlightedPlatformCode}
            platformSettings={platformSettings}
            platformSettingsLoading={platformSettingsLoading}
            platformSettingsError={platformSettingsError}
            platformSettingsSaving={platformSettingsSaving}
            onPlatformFieldChange={handlePlatformFieldChange}
            onPlatformReset={handleResetPlatformSettings}
            onPlatformSave={handleSavePlatformSettings}
          />
        )}
      </main>
    </div>
  );
};

export default App;
