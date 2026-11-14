import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";

import type { InstallWith, LoginResult, SettingsUpdatePayload, StoreAccount } from "../types";
import type { Settings } from "../settings";
import { errorMessage } from "../lib/format";
import { SettingRow, Toggle } from "./controls";
import { OwnedGamesSettings } from "./OwnedGamesSettings";

const INSTALL_WITH_LABELS: Record<InstallWith, string> = {
  auto: "The store's launcher, or its web page",
  heroic: "Heroic first (Linux)",
  browser: "Always the store's web page",
};

const fetchedAgo = (seconds: number | null): string => {
  if (!seconds) return "not fetched yet";
  const minutes = Math.round((Date.now() / 1000 - seconds) / 60);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `${hours} h ago`;
  return `${Math.round(hours / 24)} days ago`;
};

const AccountCard = ({
  account,
  busy,
  onChanged,
  onBusy,
}: {
  account: StoreAccount;
  busy: boolean;
  onChanged: (account: StoreAccount, message?: string) => void;
  onBusy: (busy: boolean) => void;
}) => {
  const [pasted, setPasted] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [showManual, setShowManual] = useState(!account.webview_login);

  const run = async (action: () => Promise<StoreAccount | void>, message?: string) => {
    setError(null);
    onBusy(true);
    try {
      const updated = await action();
      if (updated) onChanged(updated, message);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      onBusy(false);
    }
  };

  // Stays busy until the login window reports back through the "store-login" event.
  const signInWindow = async () => {
    setError(null);
    onBusy(true);
    try {
      await invoke("start_store_login", { store: account.key });
    } catch (err) {
      setError(errorMessage(err));
      onBusy(false);
    }
  };

  const openSignInPage = async () => {
    try {
      await invoke("open_store_sign_in_page", { store: account.key });
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const signInPasted = () =>
    run(async () => {
      const updated = await invoke<StoreAccount>("submit_store_login", { store: account.key, pasted });
      setPasted("");
      return updated;
    }, `Signed in to ${account.name}`);

  return (
    <section className="border-t-3 border-harbour py-5 first-of-type:border-t-0 first-of-type:pt-0">
      <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
        <h2 className="font-pixel text-2xl text-foam">{account.name}</h2>
        {account.signed_in ? (
          <p className="text-peach">
            Signed in{account.account_name ? ` as ${account.account_name}` : ""}
            {" · "}
            {account.game_count ?? 0} games, {fetchedAgo(account.fetched_at)}
          </p>
        ) : (
          <p className="text-mauve">Not signed in</p>
        )}
      </div>
      {account.signed_in && account.needs_sign_in ? (
        <p role="alert" className="mt-2 max-w-prose text-ember">
          {account.name} no longer accepts this sign-in, so the games listed are from before. Sign out and sign in
          again.
        </p>
      ) : account.signed_in && account.last_error ? (
        <p role="alert" className="mt-2 max-w-prose text-ember">
          The last refresh failed, so the games listed are from before: {account.last_error}
        </p>
      ) : null}

      {account.signed_in ? (
        <>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              className="btn-quiet"
              disabled={busy}
              onClick={() =>
                run(() => invoke<StoreAccount>("refresh_store_library", { store: account.key }), "Library refreshed")
              }
            >
              {busy ? "Working…" : "Refresh library"}
            </button>
            <button
              type="button"
              className="btn-quiet"
              disabled={busy}
              onClick={() => run(() => invoke<StoreAccount>("sign_out_store", { store: account.key }), "Signed out")}
            >
              Sign out
            </button>
          </div>
          <div className="mt-2 divide-y-2 divide-harbour/60">
            <SettingRow
              title={
                account.game_count == null
                  ? "Add every game this account owns"
                  : `Add every game this account owns (${account.game_count})`
              }
              hint="Games that are not installed get a shortcut that installs them, and go into the Steam collection “Not installed”."
            >
              <Toggle
                checked={account.enabled}
                label={`Add games from ${account.name}`}
                disabled={busy}
                onChange={(enabled) =>
                  run(() => invoke<StoreAccount>("update_store_account", { store: account.key, enabled }))
                }
              />
            </SettingRow>
            <SettingRow
              title="Leave out betas, demos and test servers"
              hint="Playtests, test servers and demos the account lists next to the games."
            >
              <Toggle
                checked={account.skip_prereleases}
                label={`Leave out betas and demos from ${account.name}`}
                disabled={busy}
                onChange={(skipPrereleases) =>
                  run(() => invoke<StoreAccount>("update_store_account", { store: account.key, skipPrereleases }))
                }
              />
            </SettingRow>
            <label className="block py-3">
              <span className="block font-bold text-foam">Games that are not installed open</span>
              <select
                className="field mt-2 max-w-md"
                value={account.install_with}
                disabled={busy}
                onChange={(e) =>
                  run(() =>
                    invoke<StoreAccount>("update_store_account", {
                      store: account.key,
                      installWith: e.target.value as InstallWith,
                    })
                  )
                }
              >
                {(Object.keys(INSTALL_WITH_LABELS) as InstallWith[])
                  .filter((value) => value !== "heroic" || account.heroic_available || account.install_with === "heroic")
                  .map((value) => (
                    <option key={value} value={value}>
                      {INSTALL_WITH_LABELS[value]}
                    </option>
                  ))}
              </select>
            </label>
          </div>
        </>
      ) : (
        <div className="mt-3">
          <div className="flex flex-wrap gap-2">
            {account.webview_login ? (
              <button type="button" className="btn-quiet" disabled={busy} onClick={signInWindow}>
                {busy ? "Waiting for sign-in…" : `Sign in to ${account.name}`}
              </button>
            ) : null}
            {account.webview_login && account.manual.kind === "paste" ? (
              <button type="button" className="link-btn text-mauve" onClick={() => setShowManual((v) => !v)}>
                {showManual ? "Hide" : "Sign in with your own browser instead"}
              </button>
            ) : null}
          </div>
          {showManual && account.manual.kind === "paste" ? (
            <div className="mt-3 max-w-prose">
              <p className="text-mauve">{account.manual.instructions}</p>
              <p className="mt-1">
                <button type="button" className="link-btn" onClick={openSignInPage}>
                  Open the sign-in page in your browser
                </button>
              </p>
              <div className="mt-2 flex flex-wrap gap-2">
                <input
                  type="password"
                  value={pasted}
                  placeholder={account.manual.placeholder}
                  onChange={(e) => setPasted(e.target.value)}
                  aria-label={`Value for ${account.name}`}
                  className="field max-w-md flex-1"
                />
                <button type="button" className="btn-quiet" disabled={busy || !pasted.trim()} onClick={signInPasted}>
                  Sign in
                </button>
              </div>
            </div>
          ) : null}
        </div>
      )}

      {error ? (
        <p role="alert" className="mt-2 max-w-prose text-ember">
          {error}
        </p>
      ) : null}
    </section>
  );
};

export const AccountsView = ({
  settings,
  onSettingsChange,
  onAccountsChanged,
  loginResult,
  onLoginResultSeen,
  librariesVersion,
}: {
  settings: Settings | null;
  onSettingsChange: (patch: SettingsUpdatePayload) => Promise<void>;
  onAccountsChanged: () => void;
  /** Goes up when libraries were fetched again in the background: the accounts are read again. */
  librariesVersion: number;
  /** The last login window result, caught by App so none is missed while this view is closed. */
  loginResult: LoginResult | null;
  onLoginResultSeen: () => void;
}) => {
  const [accounts, setAccounts] = useState<StoreAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState<Set<string>>(() => new Set());

  const load = useCallback(async () => {
    try {
      setAccounts(await invoke<StoreAccount[]>("list_store_accounts"));
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, librariesVersion]);

  // App refreshes the games itself; this view shows the result and reloads the accounts.
  useEffect(() => {
    if (!loginResult) return;
    setBusy((prev) => {
      const next = new Set(prev);
      next.delete(loginResult.store);
      return next;
    });
    if (loginResult.ok) {
      setNotice(loginResult.message);
      setError(null);
    } else {
      setNotice(null);
      setError(loginResult.message);
    }
    void load();
    onLoginResultSeen();
  }, [loginResult, load, onLoginResultSeen]);

  const setStoreBusy = (key: string, value: boolean) =>
    setBusy((prev) => {
      const next = new Set(prev);
      if (value) next.add(key);
      else next.delete(key);
      return next;
    });

  return (
    <div className="scroll-quiet h-full overflow-y-auto px-8 py-6">
      <div className="max-w-3xl">
        <h1 className="font-pixel text-3xl text-foam">Store accounts</h1>
        <p className="mt-1 max-w-prose text-mauve">
          Sign in to a store and BoilR adds every game you own there to Steam, installed or not. A game that is not
          installed opens the store’s launcher to install it. Once it is installed, BoilR swaps in the real game and
          keeps its artwork and playtime. Your sign-in is stored only on this computer.
        </p>
        {notice ? <p className="mt-3 text-peach">{notice}</p> : null}
        {error ? (
          <p role="alert" className={clsx("mt-3 max-w-prose text-ember")}>
            {error}
          </p>
        ) : null}
        <div className="mt-6">
          <OwnedGamesSettings
            settings={settings?.owned_games}
            disabled={!settings}
            onUpdate={(patch) => void onSettingsChange(patch)}
          />
        </div>
        <div className="mt-6">
          {loading ? <p className="text-mauve">Loading…</p> : null}
          {accounts.map((account) => (
            <AccountCard
              key={account.key}
              account={account}
              busy={busy.has(account.key)}
              onBusy={(value) => setStoreBusy(account.key, value)}
              onChanged={(updated, message) => {
                setAccounts((prev) => prev.map((a) => (a.key === updated.key ? updated : a)));
                setError(null);
                if (updated.notice) setNotice(`${message ? `${message}. ` : ""}${updated.notice}`);
                else if (message) setNotice(message);
                onAccountsChanged();
              }}
            />
          ))}
        </div>
      </div>
    </div>
  );
};
