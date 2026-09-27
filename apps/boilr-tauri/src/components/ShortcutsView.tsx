import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { BackupEntry, ManagedShortcut } from "../types";
import type { Settings } from "../settings";
import { errorMessage } from "../lib/format";

const formatTakenAt = (utc: string): string => {
  const date = new Date(`${utc.replace(" ", "T")}Z`);
  return Number.isNaN(date.getTime())
    ? utc
    : date.toLocaleString(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
};

const Section = ({ title, hint, children }: { title: string; hint: string; children: React.ReactNode }) => (
  <section className="border-t-3 border-harbour pt-5 first-of-type:border-t-0 first-of-type:pt-0">
    <h2 className="font-pixel text-2xl text-foam">{title}</h2>
    <p className="mt-1 max-w-prose text-mauve">{hint}</p>
    <div className="mt-3">{children}</div>
  </section>
);

/** An action that asks once more, inline, before it happens. */
const ConfirmRow = ({
  question,
  confirmLabel,
  onConfirm,
  onCancel,
  busy,
}: {
  question: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  busy: boolean;
}) => (
  <div className="mt-2 border-l-3 border-ember bg-harbour/50 px-4 py-3">
    <p className="text-peach">{question}</p>
    <div className="mt-3 flex gap-3">
      <button type="button" className="btn-primary" onClick={onConfirm} disabled={busy}>
        {busy ? "Working…" : confirmLabel}
      </button>
      <button type="button" className="link-btn" onClick={onCancel} disabled={busy}>
        Cancel
      </button>
    </div>
  </div>
);

export const ShortcutsView = ({ onSettingsChanged }: { onSettingsChanged: (settings: Settings) => void }) => {
  const [backups, setBackups] = useState<BackupEntry[] | null>(null);
  const [shortcuts, setShortcuts] = useState<ManagedShortcut[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [b, s] = await Promise.all([
        invoke<BackupEntry[]>("list_backups"),
        invoke<ManagedShortcut[]>("list_boilr_shortcuts"),
      ]);
      setBackups(b);
      setShortcuts(s);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (action: () => Promise<void>, done: string) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await action();
      setNotice(done);
      setConfirming(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const backupNow = () =>
    run(async () => setBackups(await invoke<BackupEntry[]>("create_backup")), "Backed up your current Steam shortcuts.");

  const restore = (entry: BackupEntry) =>
    run(async () => {
      setBackups(await invoke<BackupEntry[]>("restore_shortcuts", { path: entry.path }));
      await load();
    }, `Restored the shortcuts from ${formatTakenAt(entry.taken_at)}. Restart Steam to see them.`);

  const handOver = (shortcut: ManagedShortcut) =>
    run(async () => {
      onSettingsChanged(await invoke<Settings>("release_shortcut", { appId: shortcut.app_id }));
      await load();
    }, `${shortcut.name} is yours now. BoilR won't change or remove it.`);

  return (
    <div className="scroll-quiet h-full overflow-y-auto">
      <div className="mx-auto max-w-3xl space-y-8 px-6 py-6">
        <div>
          <h1 className="font-pixel text-3xl text-foam">Shortcuts</h1>
          <p className="mt-1 max-w-prose text-peach">
            Close Steam before changing anything here: Steam overwrites its shortcut list when it quits.
          </p>
        </div>
        {error ? (
          <p role="alert" className="border-l-3 border-ember px-4 py-2 text-peach">
            {error}
          </p>
        ) : null}
        {notice ? (
          <p role="status" className="border-l-3 border-flame px-4 py-2 text-foam">
            {notice}
          </p>
        ) : null}

        <Section title="Backups" hint="BoilR backs up Steam's shortcut list before every import. Restoring puts that list back.">
          <button type="button" className="btn-quiet" onClick={backupNow} disabled={busy}>
            Back up now
          </button>
          {backups === null ? (
            <p className="mt-4 text-mauve">Loading…</p>
          ) : backups.length === 0 ? (
            <p className="mt-4 text-mauve">No backups yet. One is made automatically before your first import.</p>
          ) : (
            <ul className="mt-4 divide-y-2 divide-harbour/60">
              {backups.map((entry) => {
                const key = `restore:${entry.path}`;
                return (
                  <li key={entry.path} className="py-2">
                    <div className="flex items-center justify-between gap-4">
                      <div>
                        <p className="text-foam">{formatTakenAt(entry.taken_at)}</p>
                        <p className="text-sm text-mauve">Steam account {entry.user_id}</p>
                      </div>
                      <button type="button" className="link-btn" onClick={() => setConfirming(key)} disabled={busy}>
                        Restore
                      </button>
                    </div>
                    {confirming === key ? (
                      <ConfirmRow
                        question={`Replace your current Steam shortcuts with the ones from ${formatTakenAt(entry.taken_at)}? Your current shortcuts are backed up first.`}
                        confirmLabel="Restore"
                        onConfirm={() => restore(entry)}
                        onCancel={() => setConfirming(null)}
                        busy={busy}
                      />
                    ) : null}
                  </li>
                );
              })}
            </ul>
          )}
        </Section>

        <Section
          title="Shortcuts BoilR looks after"
          hint="BoilR keeps these up to date and removes them when a game is uninstalled. Hand one over to edit it yourself in Steam; BoilR then leaves it alone."
        >
          {shortcuts === null ? (
            <p className="text-mauve">Loading…</p>
          ) : shortcuts.length === 0 ? (
            <p className="text-mauve">None yet. Games you import appear here.</p>
          ) : (
            <ul className="divide-y-2 divide-harbour/60">
              {shortcuts.map((shortcut) => {
                const key = `release:${shortcut.app_id}`;
                return (
                  <li key={shortcut.app_id} className="py-2">
                    <div className="flex items-center justify-between gap-4">
                      <p className="truncate text-foam">{shortcut.name}</p>
                      <button type="button" className="link-btn shrink-0" onClick={() => setConfirming(key)} disabled={busy}>
                        Hand over
                      </button>
                    </div>
                    {confirming === key ? (
                      <ConfirmRow
                        question={`Hand ${shortcut.name} over to you? BoilR will stop updating it, won't remove it, and won't add it again.`}
                        confirmLabel="Hand over"
                        onConfirm={() => handOver(shortcut)}
                        onCancel={() => setConfirming(null)}
                        busy={busy}
                      />
                    ) : null}
                  </li>
                );
              })}
            </ul>
          )}
        </Section>
      </div>
    </div>
  );
};
