import { useEffect, useRef, useState } from "react";

import type { Settings } from "../settings";
import type { SettingsUpdatePayload } from "../types";
import type { PlatformFieldUpdate, PlatformSettingsField, PlatformSettingsGroup } from "../lib/platformSettings";
import { groupHasChanges } from "../lib/platformSettings";
import { SettingRow, Toggle } from "./controls";
import type { NavigationStatus } from "../lib/navigation";

const FIELD_LABELS: Record<string, string> = {
  enabled: "Include this launcher",
  location: "Install folder",
  executable: "Program",
  flatpak_image: "Flatpak ID",
  installed: "Only installed games",
  create_symlinks: "Create folder shortcuts",
  safe_launch: "Launch through the launcher",
};

const Section = ({ title, hint, children }: { title: string; hint: string; children: React.ReactNode }) => (
  <section className="border-t-3 border-harbour pt-5 first-of-type:border-t-0 first-of-type:pt-0">
    <h2 className="font-pixel text-2xl text-foam">{title}</h2>
    <p className="mt-1 max-w-prose text-mauve">{hint}</p>
    <div className="mt-2 divide-y-2 divide-harbour/60">{children}</div>
  </section>
);

const TextSetting = ({
  title,
  hint,
  value,
  placeholder,
  onSave,
  secret,
}: {
  title: string;
  hint: string;
  value: string;
  placeholder: string;
  onSave: (value: string | null) => void;
  secret?: boolean;
}) => {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const changed = draft.trim() !== value;
  return (
    <div className="py-3">
      <p className="font-bold text-foam">{title}</p>
      <p className="mt-0.5 max-w-prose text-sm text-mauve">{hint}</p>
      <div className="mt-2 flex flex-wrap gap-2">
        <input
          type={secret ? "password" : "text"}
          value={draft}
          placeholder={placeholder}
          onChange={(e) => setDraft(e.target.value)}
          aria-label={title}
          className="field max-w-md flex-1"
        />
        <button type="button" className="btn-quiet" disabled={!changed} onClick={() => onSave(draft.trim() || null)}>
          Save
        </button>
      </div>
    </div>
  );
};

const PlatformField = ({
  field,
  onChange,
}: {
  field: PlatformSettingsField;
  onChange: (update: PlatformFieldUpdate) => void;
}) => {
  const label = FIELD_LABELS[field.key] ?? field.label;
  if (field.kind === "boolean") {
    return (
      <SettingRow title={label}>
        <Toggle
          checked={field.value}
          label={label}
          onChange={(value) => onChange({ key: field.key, kind: "boolean", value })}
        />
      </SettingRow>
    );
  }
  if (field.kind === "string-list") {
    return (
      <label className="block py-3">
        <span className="block font-bold text-foam">{label}</span>
        <span className="block text-sm text-mauve">One per line</span>
        <textarea
          rows={3}
          value={field.value.join("\n")}
          onChange={(e) => onChange({ key: field.key, kind: "string-list", value: e.target.value.split("\n") })}
          className="field mt-2 max-w-md"
        />
      </label>
    );
  }
  return (
    <label className="block py-3">
      <span className="block font-bold text-foam">{label}</span>
      <input
        type="text"
        value={field.value}
        placeholder={field.kind === "optional-string" ? "Found automatically" : undefined}
        onChange={(e) => onChange({ key: field.key, kind: field.kind, value: e.target.value } as PlatformFieldUpdate)}
        className="field mt-2 max-w-md"
      />
    </label>
  );
};

export const SettingsView = ({
  settings,
  onUpdate,
  error,
  platformGroups,
  platformLoading,
  platformError,
  platformSaving,
  onFieldChange,
  onReset,
  onSavePlatform,
  focusPlatform,
  navStatus,
}: {
  settings: Settings | null;
  onUpdate: (patch: SettingsUpdatePayload) => void;
  error: string | null;
  platformGroups: PlatformSettingsGroup[];
  platformLoading: boolean;
  platformError: string | null;
  platformSaving: Set<string>;
  onFieldChange: (codeName: string, update: PlatformFieldUpdate) => void;
  onReset: (codeName: string) => void;
  onSavePlatform: (codeName: string) => void;
  focusPlatform: string | null;
  navStatus: NavigationStatus;
}) => {
  const steam = settings?.steam ?? {};
  const grid = settings?.steamgrid_db ?? {};
  const refs = useRef<Record<string, HTMLDetailsElement | null>>({});

  useEffect(() => {
    if (!focusPlatform || platformLoading) return;
    const node = refs.current[focusPlatform];
    if (node) {
      node.open = true;
      node.scrollIntoView({ block: "start" });
      node.querySelector<HTMLElement>("summary")?.focus();
    }
  }, [focusPlatform, platformLoading, platformGroups]);

  if (!settings) {
    return <p className="p-6 text-mauve">Loading settings…</p>;
  }

  return (
    <div className="scroll-quiet h-full overflow-y-auto">
      <div className="mx-auto max-w-3xl space-y-8 px-6 py-6">
        <h1 className="font-pixel text-3xl text-foam">Settings</h1>
        {error ? <p className="border-l-3 border-ember px-4 py-2 text-peach">Couldn't save: {error}</p> : null}

        <Section title="Steam" hint="How BoilR works with Steam while importing.">
          <SettingRow title="Close Steam before importing" hint="Steam overwrites changes made while it's running.">
            <Toggle checked={Boolean(steam.stop_steam)} label="Close Steam before importing" onChange={(v) => onUpdate({ steam: { stop_steam: v } })} />
          </SettingRow>
          <SettingRow title="Start Steam afterwards" hint="Opens Steam again once the import is done.">
            <Toggle checked={Boolean(steam.start_steam)} label="Start Steam afterwards" onChange={(v) => onUpdate({ steam: { start_steam: v } })} />
          </SettingRow>
          <SettingRow title="Group games by launcher" hint="Adds a Steam collection per launcher, like Epic or GOG.">
            <Toggle checked={Boolean(steam.create_collections)} label="Group games by launcher" onChange={(v) => onUpdate({ steam: { create_collections: v } })} />
          </SettingRow>
          <SettingRow title="Big Picture and Steam Deck mode" hint="Picks artwork sized for the TV and Deck interface.">
            <Toggle checked={Boolean(steam.optimize_for_big_picture)} label="Big Picture and Steam Deck mode" onChange={(v) => onUpdate({ steam: { optimize_for_big_picture: v } })} />
          </SettingRow>
          <TextSetting
            title="Steam folder"
            hint="Leave empty and BoilR finds Steam by itself."
            value={steam.location ?? ""}
            placeholder="Found automatically"
            onSave={(v) => onUpdate({ steam: { location: v } })}
          />
        </Section>

        <Section title="Artwork" hint="Covers, logos and backgrounds come from SteamGridDB, a free community site.">
          <TextSetting
            title="SteamGridDB key"
            hint={
              grid.auth_key
                ? "Your key is saved."
                : "Sign in at steamgriddb.com, open Preferences, then API, and paste the key here."
            }
            value={grid.auth_key ?? ""}
            placeholder="Paste your key"
            onSave={(v) => onUpdate({ steamgrid_db: { auth_key: v } })}
            secret
          />
          <SettingRow title="Download artwork" hint="Fetches artwork for new shortcuts after importing.">
            <Toggle checked={Boolean(grid.enabled)} label="Download artwork" onChange={(v) => onUpdate({ steamgrid_db: { enabled: v } })} />
          </SettingRow>
          <SettingRow title="Prefer animated covers" hint="Uses moving artwork where it exists.">
            <Toggle checked={Boolean(grid.prefer_animated)} label="Prefer animated covers" onChange={(v) => onUpdate({ steamgrid_db: { prefer_animated: v } })} />
          </SettingRow>
          <SettingRow title="Only BoilR's games" hint="Leaves artwork on your other Steam shortcuts alone.">
            <Toggle checked={Boolean(grid.only_download_boilr_images)} label="Only BoilR's games" onChange={(v) => onUpdate({ steamgrid_db: { only_download_boilr_images: v } })} />
          </SettingRow>
          <SettingRow title="Allow adult artwork" hint="Includes artwork SteamGridDB marks as mature.">
            <Toggle checked={Boolean(grid.allow_nsfw)} label="Allow adult artwork" onChange={(v) => onUpdate({ steamgrid_db: { allow_nsfw: v } })} />
          </SettingRow>
        </Section>

        <Section title="Controller" hint="What BoilR receives from your controller. Press any button to wake it up.">
          <SettingRow title="Controllers seen" hint={navStatus.gamepadApi ? undefined : "This window has no gamepad support; only keys work."}>
            <span className="text-right text-peach">{navStatus.pads.length ? navStatus.pads.join(", ") : "None"}</span>
          </SettingRow>
          <SettingRow title="Last button">
            <span className="text-peach">{navStatus.lastInput ?? "Nothing yet"}</span>
          </SettingRow>
          <SettingRow title="Window has focus" hint="Controllers only work while this is Yes.">
            <span className="text-peach">{navStatus.windowFocused ? "Yes" : "No"}</span>
          </SettingRow>
        </Section>

        <Section title="Launchers" hint="Where BoilR looks for each launcher's games.">
          {platformError ? <p className="py-3 text-ember">{platformError}</p> : null}
          {platformLoading && platformGroups.length === 0 ? <p className="py-3 text-mauve">Loading…</p> : null}
          {platformGroups.map((group) => {
            const dirty = groupHasChanges(group);
            const saving = platformSaving.has(group.codeName);
            return (
              <details
                key={group.codeName}
                ref={(el) => {
                  refs.current[group.codeName] = el;
                }}
                className="group py-1"
              >
                <summary className="flex cursor-pointer list-none items-center justify-between py-2 font-pixel text-lg text-peach hover:text-foam">
                  {group.name}
                  <span aria-hidden className="text-mauve group-open:rotate-90">▸</span>
                </summary>
                <div className="pb-4 pl-1">
                  {group.fields.map((field) => (
                    <PlatformField key={field.key} field={field} onChange={(u) => onFieldChange(group.codeName, u)} />
                  ))}
                  <div className="mt-2 flex gap-3">
                    <button type="button" className="btn-primary" disabled={!dirty || saving} onClick={() => onSavePlatform(group.codeName)}>
                      {saving ? "Saving…" : `Save ${group.name}`}
                    </button>
                    <button type="button" className="link-btn" disabled={!dirty || saving} onClick={() => onReset(group.codeName)}>
                      Undo changes
                    </button>
                  </div>
                </div>
              </details>
            );
          })}
        </Section>
      </div>
    </div>
  );
};
