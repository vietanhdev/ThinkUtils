#!/usr/bin/env bash
#
# Build the Debian source package(s) for the Launchpad PPA.
#
# The hard constraint: Launchpad builders have NO network access. Every Rust
# dependency has to be inside the source tarball, which is what `cargo vendor`
# plus a .cargo/config.toml redirect achieves.
#
# The part that is NOT hard, and is worth stating because it usually is: npm is
# not a build dependency here. tauri.conf.json has no beforeBuildCommand and
# frontendDist points at a directory of static files, so there is no bundler and
# no node_modules to vendor. Vendoring a JS dependency tree deterministically is
# the single most painful part of packaging a typical Tauri app, and this one
# sidesteps it entirely. Do not add a bundler without re-reading this.
#
# Usage:
#   scripts/build-ppa-source.sh                      # all default series
#   scripts/build-ppa-source.sh --series noble
#   scripts/build-ppa-source.sh --ppa-rev 2
#   scripts/build-ppa-source.sh --sign-key ABCD1234
#
# Produces .dsc/.changes under build/ppa/. Upload with:
#   dput ppa:vietanhng/thinkutils build/ppa/thinkutils_*_source.changes

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
REPO_ROOT="$(pwd)"

# Ubuntu series to build for, and why these:
#
#   noble    24.04 LTS -- where most users are. Its DEFAULT rustc is 1.75, which
#            cannot even parse a v4 Cargo.lock, but the archive also carries
#            versioned rustc-1.83/cargo-1.83, so it is reachable.
#   resolute 26.04 LTS -- rustc 1.93, no special handling needed.
#   stonking 26.10.
#
# jammy (22.04) is deliberately absent, and NOT because of the toolchain: it
# ships polkit 0.105, whose JavaScript rules engine Debian and Ubuntu patched
# out. The passwordless rule this app installs would be read by nothing there,
# so every fan change would prompt for a password. Shipping a package whose
# headline feature silently degrades is worse than not shipping it.
SERIES_DEFAULT=(noble resolute stonking)

SERIES=()
PPA_REV=1
SIGN_KEY=""
SKIP_SIGN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --series) SERIES+=("$2"); shift 2 ;;
        --ppa-rev) PPA_REV="$2"; shift 2 ;;
        --sign-key) SIGN_KEY="$2"; shift 2 ;;
        --no-sign) SKIP_SIGN=1; shift ;;
        -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done
[ "${#SERIES[@]}" -gt 0 ] || SERIES=("${SERIES_DEFAULT[@]}")

for tool in dpkg-buildpackage cargo tar xz jq; do
    command -v "$tool" >/dev/null || {
        echo "ERROR: $tool is required but not installed" >&2
        echo "       sudo apt install devscripts dpkg-dev cargo xz-utils jq" >&2
        exit 1
    }
done

VERSION="$(jq -r .version package.json)"
[ -n "$VERSION" ] && [ "$VERSION" != "null" ] || { echo "ERROR: no version in package.json" >&2; exit 1; }

OUT="${REPO_ROOT}/build/ppa"
WORK="${OUT}/work"
rm -rf "$WORK"
mkdir -p "$WORK"

PKG="thinkutils"
SRCDIR="${WORK}/${PKG}-${VERSION}"
ORIG="${OUT}/${PKG}_${VERSION}.orig.tar.xz"

echo "==> Building PPA source for ${PKG} ${VERSION}"
echo "    series: ${SERIES[*]}"

# --- 1. Clean tree ---------------------------------------------------------
# git archive rather than cp: it respects .gitignore by construction, so
# target/, node_modules/ and build output cannot leak into the tarball.
echo "==> Exporting a clean tree"
mkdir -p "$SRCDIR"
git archive HEAD | tar -x -C "$SRCDIR"

# --- 2. Vendor the Rust dependencies --------------------------------------
echo "==> Vendoring cargo dependencies (this is the offline-build requirement)"
(
    cd "${SRCDIR}/src-tauri"
    mkdir -p .cargo
    cargo vendor --versioned-dirs --locked vendor > .cargo/config.toml
)
VENDOR_MB=$(du -sm "${SRCDIR}/src-tauri/vendor" | cut -f1)
echo "    vendored: ${VENDOR_MB} MB (~77 MB in the compressed tarball)"

# About three quarters of that is Windows-only crates -- windows-*, winapi-*,
# webview2-com-sys -- in a package that only ever builds for Linux. Deleting
# them looks like an easy 73% saving and DOES NOT WORK: cargo requires a
# vendored source for every crate in Cargo.lock, including platform-gated ones
# it will never compile. Tested directly; `cargo check --offline` fails resolving
# chrono once they are gone. Do not retry it.

# --- 3. Deterministic .orig tarball ---------------------------------------
# One .orig per version, shared by every series: the first upload carries it
# (-sa) and later ones reference it (-sd). If two runs produced byte-different
# tarballs for the same version, Launchpad rejects the second with "already
# exists, but uploaded version has different contents". Hence the fixed mtime,
# sorted entries, numeric owner, and single-threaded xz -- xz -T0 is not
# byte-reproducible.
if [ -f "$ORIG" ]; then
    echo "==> Reusing existing ${ORIG##*/} (required: all series must share one)"
else
    echo "==> Building deterministic orig tarball"
    tar --sort=name \
        --mtime='2020-01-01 00:00:00Z' \
        --owner=0 --group=0 --numeric-owner \
        --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime \
        --exclude='./debian' \
        --exclude='./node_modules' \
        --exclude='./docs' \
        --exclude='./screenshots' \
        -C "$WORK" -cf - "${PKG}-${VERSION}" \
        | xz -6 -T1 > "$ORIG"
    echo "    $(du -h "$ORIG" | cut -f1)  ${ORIG##*/}"
fi

# --- 4. Per-series source packages ----------------------------------------
first=1
for series in "${SERIES[@]}"; do
    echo "==> ${series}"
    rm -rf "${SRCDIR}/debian"
    cp -r packaging/ppa/debian "${SRCDIR}/debian"

    # noble's default rustc is 1.75 and cannot parse a v4 Cargo.lock. The
    # archive carries versioned packages, so name them explicitly rather than
    # using an alternative like `rustc (>= 1.83) | rustc-1.83` -- sbuild picks
    # the FIRST installable alternative, which varies per series and makes the
    # result non-deterministic.
    case "$series" in
        noble)
            rust_deps=" rustc-1.83,\n cargo-1.83,"
            rust_path='export PATH := /usr/lib/rust-1.83/bin:$(PATH)'
            ;;
        *)
            rust_deps=" rustc,\n cargo,"
            rust_path=''
            ;;
    esac

    python3 - "$SRCDIR" "$rust_deps" "$rust_path" <<'PY'
import sys
srcdir, rust_deps, rust_path = sys.argv[1], sys.argv[2], sys.argv[3]
ctl = open(f"{srcdir}/debian/control.in").read()
ctl = ctl.replace("@RUST_BUILD_DEPS@", rust_deps.replace("\\n", "\n"))
open(f"{srcdir}/debian/control", "w").write(ctl)
import os
os.remove(f"{srcdir}/debian/control.in")

rules = open(f"{srcdir}/debian/rules").read()
rules = rules.replace("@RUST_PATH_LINE@", rust_path)
open(f"{srcdir}/debian/rules", "w").write(rules)
PY
    chmod +x "${SRCDIR}/debian/rules"

    # Written directly rather than via dch: dch wants an existing changelog and
    # a configured maintainer identity, neither of which a CI runner has.
    cat > "${SRCDIR}/debian/changelog" <<EOF
${PKG} (${VERSION}-0ppa${PPA_REV}~${series}1) ${series}; urgency=medium

  * Release ${VERSION}.

 -- Viet Anh Nguyen <vietanh.dev@gmail.com>  $(date -R)
EOF

    # -sa on the first series includes the .orig; -sd on later ones references
    # the one already uploaded.
    if [ "$first" = "1" ]; then include="-sa"; first=0; else include="-sd"; fi

    sign_args=(-us -uc)
    if [ "$SKIP_SIGN" = "0" ] && [ -n "$SIGN_KEY" ]; then
        sign_args=("--sign-key=${SIGN_KEY}")
    fi

    ( cd "$SRCDIR" && dpkg-buildpackage -S "$include" -d "${sign_args[@]}" )
done

mkdir -p "$OUT"
mv "${WORK}"/*.dsc "${WORK}"/*.changes "${WORK}"/*.debian.tar.* "$OUT"/ 2>/dev/null || true

echo
echo "==> Done. Artifacts in ${OUT}"
ls -1 "$OUT" | sed 's/^/    /'
echo
echo "Upload with:"
echo "    dput ppa:vietanhng/thinkutils ${OUT}/${PKG}_*_source.changes"
echo
echo "A Launchpad upload cannot be undone -- a version can be superseded, never"
echo "deleted. Check the .changes file before running dput."
