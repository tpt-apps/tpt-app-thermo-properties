# TPT Thermo Properties - task runner.
# The pipeline logic lives in build.ps1 (Windows) / build.sh (unix); these
# recipes are thin wrappers so `just` works everywhere.

set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

# Build the free WASM edition and report the gzip payload size.
build:
    pwsh -NoLogo -NoProfile -File build.ps1

# Build the Pro edition: cycle analysis/library/export + trunk bundle + desktop exe.
pro:
    pwsh -NoLogo -NoProfile -File build.ps1 -Pro

# Re-run only wasm-bindgen + copy (after an existing release build).
bindgen:
    pwsh -NoLogo -NoProfile -File build.ps1 -SkipBuild

# Engine validation suite (reference property anchors) + cycle units, then the app.
test:
    cargo test -p tpt-thermo-engine
    cargo check --target wasm32-unknown-unknown -p tpt-app-thermo-properties

# Type-check the wasm lib (free + pro) and the desktop shell.
check:
    cargo check --target wasm32-unknown-unknown -p tpt-app-thermo-properties
    cargo check --target wasm32-unknown-unknown --features pro -p tpt-app-thermo-properties
    cargo check -p tpt-thermo-desktop

# Live-reload standalone dev server (index.html mounts via #tpt-appfront-root).
dev:
    trunk serve

# Remove build outputs.
clean:
    cargo clean
    -rd /s /q dist 2>nul || rm -rf dist
