#!/usr/bin/env bash
# Self-contained test for scripts/install.sh prebuilt-binary behaviour.
# Serves fake release layouts over a localhost python http.server; uses temp
# HOME/prefix and a fake `cargo` that fails the test if invoked unexpectedly.
# Usage: scripts/tests/install_prebuilt_test.sh
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
INSTALL_SRC="${HERE}/../install.sh"
T="$(mktemp -d)"
SRV_PID=""
cleanup() { [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT

command -v python3 >/dev/null || { echo "SKIP: python3 missing"; exit 0; }

# Standalone copy of the script: no Cargo.toml in its parent => "not a checkout".
mkdir -p "$T/standalone/scripts" "$T/www" "$T/fakebin"
cp "$INSTALL_SRC" "$T/standalone/scripts/install.sh"

case "$(uname -s)" in
    Linux*) OSPART="unknown-linux-gnu" ;; Darwin*) OSPART="apple-darwin" ;;
    *) echo "SKIP: unsupported OS"; exit 0 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) ARCH=x86_64 ;; aarch64|arm64) ARCH=aarch64 ;; *) ARCH="$(uname -m)" ;;
esac
TRIPLE="${ARCH}-${OSPART}"
ASSET="bilterm-${TRIPLE}.tar.gz"

cat > "$T/fakebin/cargo" <<EOC
#!/bin/sh
echo "\$@" >> "$T/cargo.called"
exit 1
EOC
chmod +x "$T/fakebin/cargo"

mkbin() { # dir version
    mkdir -p "$1"; local b="$T/pkg-$2"; mkdir -p "$b"
    printf '#!/bin/sh\necho "bilterm %s"\n' "$2" > "$b/bilterm"; chmod +x "$b/bilterm"
    tar -czf "$1/${ASSET}" -C "$b" bilterm
}
sums() { # dir
    ( cd "$1" && if command -v sha256sum >/dev/null; then sha256sum "${ASSET}"; else shasum -a 256 "${ASSET}"; fi > SHA256SUMS )
}

# Layout: releases/download/<tag>/<asset>; releases/latest/download -> latest tag (if it has asset)
R="$T/www/releases"
mkdir -p "$R/download" "$T/www/api"
PORT=$((20000 + RANDOM % 20000))
BASE="http://127.0.0.1:${PORT}"
set_api() { # json body
    mkdir -p "$T/www/api"; printf '%s' "$1" > "$T/www/api/releases"
}
api_release() { # tag prerelease hasasset
    local a=""
    [ "$3" = 1 ] && a="{\"name\":\"${ASSET}\",\"browser_download_url\":\"${BASE}/releases/download/$1/${ASSET}\"}"
    printf '{"url":"x","tag_name":"%s","name":"%s","draft":false,"prerelease":%s,"assets":[%s]}' "$1" "$1" "$2" "$a"
}

# python http.server serves "api/releases?per_page=30" by path; query is ignored.
( cd "$T/www" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 ) &
SRV_PID=$!
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    curl -fs "${BASE}/" >/dev/null 2>&1 && break; sleep 0.25
done

FAILS=0; N=0
ok()   { N=$((N+1)); echo "ok   - $1"; }
fail() { N=$((N+1)); FAILS=$((FAILS+1)); echo "FAIL - $1"; [ -f "$T/out" ] && sed 's/^/       /' "$T/out"; }

run_install() { # args...; sets RC, output in $T/out
    rm -rf "${T:?}/home" "${T:?}/prefix" "${T:?}/cargo.called"; mkdir -p "$T/home"
    HOME="$T/home" PATH="$T/fakebin:$PATH" \
    BILTERM_RELEASE_BASE_URL="${BASE}/releases" BILTERM_API_BASE_URL="${BASE}/api" \
        bash "$T/standalone/scripts/install.sh" --prefix "$T/prefix" --no-desktop --force "$@" >"$T/out" 2>&1
    RC=$?
}
reset_www() { rm -rf "$R" "$T/www/api"; mkdir -p "$R/download" "$R/latest"; }
# latest/download must mirror the newest release; emulate with a copy.
publish() { # tag version [nosums]
    mkbin "$R/download/$1" "$2"; [ "${3:-}" = nosums ] || sums "$R/download/$1"
}
latest_from() { rm -rf "$R/latest"; mkdir -p "$R/latest"; cp -R "$R/download/$1" "$R/latest/download"; }
installed_version() { "$T/prefix/bin/bilterm" 2>/dev/null; }

# 1. latest has asset
reset_www; publish v2.0.0 2.0.0; latest_from v2.0.0
set_api "[$(api_release v2.0.0 false 1)]"
run_install
if [ $RC -eq 0 ] && [ "$(installed_version)" = "bilterm 2.0.0" ] && grep -q 'SHA256 verified' "$T/out" && [ ! -f "$T/cargo.called" ]; then ok "latest has asset"; else fail "latest has asset"; fi

# 2. latest missing, older release has it
reset_www; publish v1.0.0 1.0.0
set_api "[$(api_release v3.0.0 false 0),$(api_release v2.0.0 false 0),$(api_release v1.0.0 false 1)]"
run_install
if [ $RC -eq 0 ] && [ "$(installed_version)" = "bilterm 1.0.0" ] && grep -q 'latest release has no prebuilt binary' "$T/out" && grep -q 'v1.0.0' "$T/out" && [ ! -f "$T/cargo.called" ]; then ok "fallback to older release with warning"; else fail "fallback to older release with warning"; fi

# 2b. prefers non-prerelease over a newer prerelease
reset_www; publish v1.0.0 1.0.0; publish v2.0.0-rc1 2.0.0
set_api "[$(api_release v3.0.0 false 0),$(api_release v2.0.0-rc1 true 1),$(api_release v1.0.0 false 1)]"
run_install
if [ $RC -eq 0 ] && [ "$(installed_version)" = "bilterm 1.0.0" ]; then ok "prefers stable over prerelease"; else fail "prefers stable over prerelease"; fi

# 2c. only a prerelease has it
reset_www; publish v2.0.0-rc1 2.0.0
set_api "[$(api_release v3.0.0 false 0),$(api_release v2.0.0-rc1 true 1)]"
run_install
if [ $RC -eq 0 ] && [ "$(installed_version)" = "bilterm 2.0.0" ] && grep -q 'v2.0.0-rc1' "$T/out"; then ok "falls back to prerelease"; else fail "falls back to prerelease"; fi

# 3. none have it
reset_www
set_api "[$(api_release v3.0.0 false 0),$(api_release v2.0.0 false 0)]"
run_install
if [ $RC -ne 0 ] && grep -q -- '--from-source' "$T/out" && grep -q -- '--tag' "$T/out" && [ ! -f "$T/cargo.called" ] && [ ! -e "$T/prefix/bin/bilterm" ]; then ok "no binary anywhere: non-zero, no cargo"; else fail "no binary anywhere: non-zero, no cargo"; fi

# 4. --tag missing
reset_www; publish v1.0.0 1.0.0
set_api "[$(api_release v1.0.0 false 1)]"
run_install --tag v9.9.9
if [ $RC -ne 0 ] && grep -q 'v9.9.9' "$T/out" && [ ! -f "$T/cargo.called" ]; then ok "--tag missing: error, no fallback"; else fail "--tag missing: error, no fallback"; fi

# 4b. --tag present
run_install --tag v1.0.0
if [ $RC -eq 0 ] && [ "$(installed_version)" = "bilterm 1.0.0" ]; then ok "--tag present"; else fail "--tag present"; fi

# 5. --from-source reaches cargo
reset_www
run_install --from-source
if [ $RC -ne 0 ] && [ -f "$T/cargo.called" ] && grep -q 'install --git' "$T/cargo.called"; then ok "--from-source invokes cargo"; else fail "--from-source invokes cargo"; fi

# 6. checksum mismatch
reset_www; publish v2.0.0 2.0.0; latest_from v2.0.0
printf '%064d  %s\n' 0 "$ASSET" > "$R/latest/download/SHA256SUMS"
set_api "[$(api_release v2.0.0 false 1)]"
run_install
if [ $RC -ne 0 ] && grep -q 'SHA256 mismatch' "$T/out" && [ ! -e "$T/prefix/bin/bilterm" ]; then ok "checksum mismatch fails"; else fail "checksum mismatch fails"; fi

# 6b. missing SHA256SUMS warns and continues
reset_www; publish v2.0.0 2.0.0 nosums; latest_from v2.0.0
run_install
if [ $RC -eq 0 ] && grep -q 'No SHA256SUMS' "$T/out"; then ok "absent SHA256SUMS warns, continues"; else fail "absent SHA256SUMS warns, continues"; fi

# 7. dry-run touches nothing, no network required for latest
reset_www
run_install --dry-run
if [ $RC -eq 0 ] && [ ! -e "$T/prefix/bin/bilterm" ] && [ ! -f "$T/cargo.called" ]; then ok "--dry-run"; else fail "--dry-run"; fi

echo "$((N-FAILS))/$N passed"
[ "$FAILS" -eq 0 ]
