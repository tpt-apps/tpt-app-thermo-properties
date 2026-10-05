#!/usr/bin/env sh
# TPT Thermo Properties - build pipeline (macOS/Linux).
#
# Free edition (default):  ./build.sh
# Pro desktop edition:     ./build.sh --pro
#   (adds trunk build --release --features pro + the native webview exe)
#
# Env:
#   TPT_WEB_REPO  path to the web repo; the bundle is copied to
#                 $TPT_WEB_REPO/public/apps/thermo-properties/ (glue named
#                 thermo-properties.js, wasm keeps its _bg.wasm name),
#                 otherwise dist/web in this repo.

set -eu
cd "$(dirname "$0")"

CRATE=tpt-app-thermo-properties
TARGET=wasm32-unknown-unknown
OUT_NAME=thermo-properties

PRO=0
for arg in "$@"; do
  case "$arg" in
    --pro) PRO=1 ;;
    *) echo "usage: build.sh [--pro]" >&2; exit 2 ;;
  esac
done

if [ -n "${TPT_WEB_REPO:-}" ]; then
  DEST="$TPT_WEB_REPO/public/apps/thermo-properties"
else
  DEST="dist/web"
fi

echo "== TPT Thermo Properties build =="
echo " edition: $([ "$PRO" = 1 ] && echo 'pro (cycle analysis + saved fluid library + calc-sheet export)' || echo 'free (single-point property lookup)')"
echo " dest:    $DEST"

if [ "$PRO" = 1 ]; then
  echo "-- trunk build --release --features pro"
  trunk build --release --features pro
  echo "-- cargo build --release -p tpt-thermo-desktop"
  cargo build --release -p tpt-thermo-desktop
  echo "-- done (exe: target/release/tpt-thermo-properties-pro.exe; dist/ was NOT copied to $DEST)"
  exit 0
fi

echo "-- cargo build --release --target $TARGET"
cargo build --release --target "$TARGET" -p "$CRATE"

WASM="target/$TARGET/release/tpt_app_thermo_properties.wasm"
[ -f "$WASM" ] || { echo "missing $WASM" >&2; exit 1; }

echo "-- wasm-bindgen --target web --out-name $OUT_NAME"
mkdir -p "$DEST"
wasm-bindgen --target web --out-name "$OUT_NAME" --out-dir "$DEST" "$WASM"

RAW=$(wc -c < "$DEST/${OUT_NAME}_bg.wasm" | tr -d ' ')
GZ=$(gzip -c "$DEST/${OUT_NAME}_bg.wasm" | wc -c | tr -d ' ')
echo "-- ${OUT_NAME}_bg.wasm: $RAW bytes raw, $GZ bytes gzipped"
echo "-- done"
