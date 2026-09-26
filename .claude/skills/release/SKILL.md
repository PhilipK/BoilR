---
name: release
description: Cut a BoilR release. Philip only.
disable-model-invocation: true
---

1. On an up-to-date `main` with green CI, bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` updates, regenerate `flatpak/cargo-lock.json` (`flatpak/update-cargo-lock-json.sh`), and bump the release entry in `flatpak/io.github.philipk.boilr.appdata.xml`.
2. Write release notes from merged PRs since the last tag (`git log v.<last>..HEAD --merges` and `gh pr list --state merged`), grouped as Fixes, Features, Platform changes. Credit contributors by handle.
3. Commit through a PR, merge it, then tag: `git tag v.X.Y.Z && git push origin v.X.Y.Z` (dot after `v`). This triggers `release_on_v_tag.yml`, which builds Linux and Windows binaries into a draft prerelease.
4. When the builds finish, paste the notes into the draft release, check both binaries are attached, and publish it. Philip publishes; stop and tell him when the draft is ready.
5. Flathub has its own manifest repo; after publishing, open or update the Flathub PR bumping the tag and commit hash.
