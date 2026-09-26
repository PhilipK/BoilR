---
name: release
description: Cut a BoilR release. Philip only.
disable-model-invocation: true
---

1. On an up-to-date `main` with green CI, bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` updates, regenerate `flatpak/cargo-lock.json` (`flatpak/update-cargo-lock-json.sh`), and bump the release entry in `flatpak/io.github.philipk.boilr.appdata.xml`.
2. Write release notes from merged PRs since the last tag (`git log v.<last>..HEAD --merges` and `gh pr list --state merged`), grouped as Fixes, Features, Platform changes. Credit contributors by handle.
3. Commit through a PR, merge it, then tag: `git tag v.X.Y.Z && git push origin v.X.Y.Z` (dot after `v`). This triggers `release_on_v_tag.yml`, which builds Linux and Windows binaries into a draft prerelease.
4. When the builds finish, the release is already public as a prerelease. Rename the assets to `linux_BoilR` and `windows_BoilR.exe` (the workflow uploads them as `boilr`/`boilr.exe`; older links expect the old names), set the notes, and hand over to Philip to untick prerelease.
5. **Flathub.** The shipped manifest lives in `flathub/io.github.philipk.boilr`, not in `flatpak/` here (the in-repo copy has drifted and is not what users get). In that repo, on a branch: set `commit:` to the full SHA of the release tag, replace `cargo-lock.json` with this release's `flatpak/cargo-lock.json`, and keep `runtime-version` on a supported freedesktop branch. Open a PR; flathubbot test-builds it automatically. Philip merges it, which publishes the update.
   - The Flathub linter rejects new read access to another Flatpak's data folder (`~/.var/app/<id>/...`) unless Flathub admins grant a `finish-args-flatpak-appdata-folder` exception. Leave such paths out of the release PR and request the exception separately.
