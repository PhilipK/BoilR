"""Fail when the Flatpak's offline sources don't match the lock files they were generated from.

- flatpak/cargo-lock.json vs Cargo.lock (egui app): fix with flatpak/update-cargo-lock-json.sh
- apps/boilr-tauri/flatpak/cargo-sources.json vs apps/boilr-tauri/Cargo.lock and
  apps/boilr-tauri/flatpak/node-sources.json vs apps/boilr-tauri/package-lock.json (Tauri app):
  fix with apps/boilr-tauri/flatpak/update-sources.sh
"""
import json
import sys
import tomllib


def cargo_wanted(lock_path):
    with open(lock_path, "rb") as f:
        lock = tomllib.load(f)
    return {
        f"{p['name']}-{p['version']}": p.get("checksum")
        for p in lock["package"]
        if p.get("source", "").startswith("registry+")
    }


def cargo_vendored(sources_path):
    with open(sources_path) as f:
        sources = json.load(f)
    return {
        s["dest"].rsplit("/", 1)[-1]: s.get("sha256")
        for s in sources
        if s.get("type") == "archive" and s.get("dest", "").startswith("cargo/vendor/")
    }


def npm_wanted(lock_path):
    with open(lock_path) as f:
        packages = json.load(f)["packages"]
    return {v["resolved"]: v.get("integrity") for k, v in packages.items() if k and "resolved" in v}


def npm_vendored(sources_path):
    with open(sources_path) as f:
        sources = json.load(f)
    # Integrity is checked by flatpak-builder itself; here only the set of packages matters.
    wanted = {}
    for s in sources:
        if s.get("type") in ("file", "archive") and "url" in s:
            wanted[s["url"]] = None
    return wanted


def check(name, wanted, vendored, fix, compare_checksums=True):
    missing = sorted(k for k in wanted if k not in vendored)
    stale = sorted(k for k in vendored if k not in wanted)
    mismatched = (
        sorted(k for k in wanted if k in vendored and wanted[k] != vendored[k])
        if compare_checksums
        else []
    )
    for label, items in (("missing", missing), ("checksum mismatch", mismatched), ("stale", stale)):
        for item in items:
            print(f"{name}: {label}: {item}")
    if missing or mismatched or stale:
        print(f"{name} is out of sync; run {fix}")
        return False
    print(f"{name} in sync ({len(wanted)} packages)")
    return True


results = [
    check(
        "flatpak/cargo-lock.json",
        cargo_wanted("Cargo.lock"),
        cargo_vendored("flatpak/cargo-lock.json"),
        "flatpak/update-cargo-lock-json.sh",
    ),
    check(
        "apps/boilr-tauri/flatpak/cargo-sources.json",
        cargo_wanted("apps/boilr-tauri/Cargo.lock"),
        cargo_vendored("apps/boilr-tauri/flatpak/cargo-sources.json"),
        "apps/boilr-tauri/flatpak/update-sources.sh",
    ),
    check(
        "apps/boilr-tauri/flatpak/node-sources.json",
        npm_wanted("apps/boilr-tauri/package-lock.json"),
        npm_vendored("apps/boilr-tauri/flatpak/node-sources.json"),
        "apps/boilr-tauri/flatpak/update-sources.sh",
        compare_checksums=False,
    ),
]
sys.exit(0 if all(results) else 1)
