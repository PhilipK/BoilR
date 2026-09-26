"""Fail when flatpak/cargo-lock.json does not vendor exactly the crates in Cargo.lock.

Fix a failure by running flatpak/update-cargo-lock-json.sh.
"""
import json
import sys
import tomllib

with open("Cargo.lock", "rb") as f:
    lock = tomllib.load(f)
with open("flatpak/cargo-lock.json") as f:
    sources = json.load(f)

wanted = {
    f"{p['name']}-{p['version']}": p.get("checksum")
    for p in lock["package"]
    if p.get("source", "").startswith("registry+")
}
vendored = {
    s["dest"].rsplit("/", 1)[-1]: s.get("sha256")
    for s in sources
    if s.get("type") == "archive" and s.get("dest", "").startswith("cargo/vendor/")
}

missing = sorted(k for k in wanted if k not in vendored)
mismatched = sorted(k for k in wanted if k in vendored and wanted[k] != vendored[k])
stale = sorted(k for k in vendored if k not in wanted)

for label, items in (("missing", missing), ("checksum mismatch", mismatched), ("stale", stale)):
    for item in items:
        print(f"{label}: {item}")

if missing or mismatched or stale:
    print("flatpak/cargo-lock.json is out of sync with Cargo.lock; run flatpak/update-cargo-lock-json.sh")
    sys.exit(1)
print(f"flatpak/cargo-lock.json in sync ({len(wanted)} crates)")
