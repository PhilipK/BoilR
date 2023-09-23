#!/bin/bash
curl https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py --output flatpak-cargo-generator.py
cd ..
pip install toml
python3 ./flatpak/flatpak-cargo-generator.py ./Cargo.lock -o ./flatpak/cargo-lock.json
rm ./flatpak/flatpak-cargo-generator.py