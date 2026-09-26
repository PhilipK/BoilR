---
name: review-pr
description: Review a BoilR pull request and reach a verdict (merge, request changes, close, hand to Philip). Use for community PRs, dependabot bumps, and the maintainer's own PRs before merging.
---

Read the full diff (`gh pr diff <n>`), the PR description, linked issues, CI status, and the existing discussion. Then check it out locally and run the checks in `CLAUDE.md` "Build and verify", unless it is a lockfile-only dependabot bump with green CI.

## Checklist

Apply every rule; note each failure with file and line.

- **Safety** (community PRs): new dependencies are well known and needed; no new network calls, process spawning, or file writes outside Steam and BoilR config folders that the PR does not explain; no changes to `.github/workflows/`, `build.rs`, or release scripts without a clear reason. Any doubt here means hand to Philip.
- **Settings compatibility**: no platform `code_name()` changes; new settings have defaults in `src/defaultconfig.toml`; renamed settings are migrated in `src/migration.rs`.
- **Error handling**: no `unwrap`/`expect`/`panic`/indexing (clippy denies them); errors surface to the user instead of silently returning zero games.
- **Platform gating**: Windows-only and Linux-only code sits behind the right `cfg`; CI's Windows job is green if Windows code changed.
- **Flatpak**: `Cargo.lock` changes need a regenerated `flatpak/cargo-lock.json`; the `flatpak_lock_sync` CI job goes red when they drift. For dependabot and community PRs, a red `flatpak_lock_sync` is the only failure that does not block merging: merge with every other check green, then batch the regeneration into one follow-up PR of your own per run.
- **UI thread**: new work that does IO or network runs off the UI thread, not via `block_on` in `update()`.
- **History of removed code**: for every line the PR deletes or rewrites, find why it was added (`git log -L <start>,<end>:<file>` or `git blame`, then the linked PR or issue). A removed line that was itself a fix is a regression risk; name the old issue and check the PR still covers it. (#479 silently reverted the Steam Deck scaling fix from #417.)
- **Scope**: one concern per PR. A PR mixing a bug fix with renames and formatting across many files is reviewed per concern; ask for a split if the concerns cannot be judged together.
- **Steam formats**: code that reads or writes Steam files matches what current Steam writes (see `CLAUDE.md`).

## Verdicts

- **Merge**: checklist passes, CI green, and it is a dependabot bump or a small community fix (roughly under 100 changed lines, correctness checkable without the GUI or Windows). Squash merge with a clear title. Thank community authors in a short comment.
- **Request changes**: comment with the specific failures and how to fix them. Be kind; these are volunteers.
- **Close**: superseded, abandoned (no author response 30 days after changes were requested), or out of scope. Say why and credit the work.
- **Hand to Philip**: larger PRs, GUI or Windows-dependent changes, and the maintainer's own PRs. Label `needs-philip`, comment that it awaits the owner's decision, and give Philip a two-line summary plus your recommendation in the digest.

Dependabot: merge patch and minor bumps with green CI. Major bumps and anything touching `egui`/`eframe` get a real review and usually `needs-testing`. Close dependabot PRs superseded by a newer bump of the same crate.
