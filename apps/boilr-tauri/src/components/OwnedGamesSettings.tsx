import type { MultiStore, NameLabel, OwnedGamesSettings as OwnedSettings } from "../settings";
import type { SettingsUpdatePayload } from "../types";
import { SettingRow, Toggle } from "./controls";

/** Stores in the order BoilR uses when none is set; keys as in `config.toml`. */
export const STORE_NAMES: Record<string, string> = {
  gog: "GOG",
  epic: "Epic Games Store",
  amazon: "Amazon Games",
  ea: "EA app",
  battlenet: "Battle.net",
  humble: "Humble Bundle",
  itch: "itch.io",
};

const DEFAULT_ORDER = Object.keys(STORE_NAMES);

const MULTI_STORE_LABELS: Record<MultiStore, string> = {
  preferred: "One entry, from the first store in the list below",
  label_store: "One entry per store, named after its store",
  all: "One entry per store, same name",
};

const NAME_LABELS: Record<NameLabel, string> = {
  none: "Only the title",
  not_installed: "Title (Not installed)",
  store: "Title (GOG)",
  store_and_not_installed: "Title (GOG, not installed)",
};

/** The full store order: the saved one, then any store it leaves out. */
export const fullStoreOrder = (saved: string[] | undefined): string[] => {
  const order = (saved ?? []).filter((key) => key in STORE_NAMES);
  return [...order, ...DEFAULT_ORDER.filter((key) => !order.includes(key))];
};

/** How games an account owns but that are not installed show up in Steam (`[owned_games]`). */
export const OwnedGamesSettings = ({
  settings,
  disabled,
  onUpdate,
}: {
  settings: OwnedSettings | undefined;
  disabled?: boolean;
  onUpdate: (patch: SettingsUpdatePayload) => void;
}) => {
  const owned = settings ?? {};
  const update = (patch: NonNullable<SettingsUpdatePayload["owned_games"]>) => onUpdate({ owned_games: patch });
  const order = fullStoreOrder(owned.store_order);
  const move = (index: number, by: number) => {
    const next = [...order];
    const [store] = next.splice(index, 1);
    next.splice(index + by, 0, store);
    update({ store_order: next });
  };
  const confirmAbove = owned.confirm_above ?? 100;

  return (
    <section className="border-b-3 border-harbour pb-5">
      <h2 className="font-pixel text-2xl text-foam">How they appear in Steam</h2>
      <p className="mt-1 max-w-prose text-mauve">
        For games an account owns that are not installed. Installed games always appear once, under their own name.
      </p>
      <div className="mt-2 divide-y-2 divide-harbour/60">
        <SettingRow title="Skip games Steam already has" hint="Leaves out a game Steam has installed itself.">
          <Toggle
            checked={owned.skip_games_in_steam ?? true}
            label="Skip games Steam already has"
            disabled={disabled}
            onChange={(v) => update({ skip_games_in_steam: v })}
          />
        </SettingRow>
        <SettingRow
          title="Skip games installed from another store"
          hint="Leaves out a game you have installed from another store or launcher, matched by name."
        >
          <Toggle
            checked={owned.skip_installed_elsewhere ?? true}
            label="Skip games installed from another store"
            disabled={disabled}
            onChange={(v) => update({ skip_installed_elsewhere: v })}
          />
        </SettingRow>
        <label className="block py-3">
          <span className="block font-bold text-foam">A game you own in several stores</span>
          <select
            className="field mt-2 max-w-md"
            value={owned.multi_store ?? "preferred"}
            disabled={disabled}
            onChange={(e) => update({ multi_store: e.target.value as MultiStore })}
          >
            {(Object.keys(MULTI_STORE_LABELS) as MultiStore[]).map((value) => (
              <option key={value} value={value}>
                {MULTI_STORE_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <div className="py-3">
          <p className="font-bold text-foam">Preferred stores</p>
          <p className="mt-0.5 text-sm text-mauve">Which store's entry stays when you own a game in several.</p>
          <ol className="mt-2 max-w-md space-y-1">
            {order.map((store, index) => (
              <li key={store} className="flex items-center gap-2">
                <span className="w-6 text-right text-mauve">{index + 1}.</span>
                <span className="flex-1 text-peach">{STORE_NAMES[store] ?? store}</span>
                <button
                  type="button"
                  className="btn-quiet px-2"
                  aria-label={`Move ${STORE_NAMES[store] ?? store} up`}
                  disabled={disabled || index === 0}
                  onClick={() => move(index, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  className="btn-quiet px-2"
                  aria-label={`Move ${STORE_NAMES[store] ?? store} down`}
                  disabled={disabled || index === order.length - 1}
                  onClick={() => move(index, 1)}
                >
                  ↓
                </button>
              </li>
            ))}
          </ol>
        </div>
        <label className="block py-3">
          <span className="block font-bold text-foam">Name in Steam</span>
          <span className="mt-0.5 block text-sm text-mauve">
            Steam has no "not installed" state for these games, so the name can say it. The label goes away once the
            game is installed; artwork and playtime stay.
          </span>
          <select
            className="field mt-2 max-w-md"
            value={owned.name_label ?? "none"}
            disabled={disabled}
            onChange={(e) => update({ name_label: e.target.value as NameLabel })}
          >
            {(Object.keys(NAME_LABELS) as NameLabel[]).map((value) => (
              <option key={value} value={value}>
                {NAME_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <SettingRow
          title="Ask before adding many"
          hint="Asks before an import that adds more not-installed games than this. 0 never asks."
        >
          <input
            type="number"
            min={0}
            step={10}
            className="field w-24 text-right"
            aria-label="Ask before adding more than this many not-installed games"
            value={confirmAbove}
            disabled={disabled}
            onChange={(e) => {
              const value = Number.parseInt(e.target.value, 10);
              if (Number.isFinite(value) && value >= 0) update({ confirm_above: value });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
};
