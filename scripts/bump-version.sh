#!/usr/bin/env bash
#
# Bump the version everywhere it is declared, or verify they all agree.
#
#   ./scripts/bump-version.sh 0.1.11     # set every declaration
#   ./scripts/bump-version.sh --check    # verify they agree (used by CI)
#
# CLAUDE.md documents four files that must be edited together, plus a cargo check
# to refresh Cargo.lock. Doing that by hand is how a tree ends up half-bumped:
# the package filename says one version and the About dialog says another, or the
# tag disagrees with the manifests and release.yml rejects it after a full build.
#
# Every edit is asserted to have changed something. A sed that silently matches
# nothing is the specific failure this script exists to prevent -- it is
# indistinguishable from success, right up until release.
#
# Approach adapted from the sibling Bulwark project (Apache-2.0).

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

CHECK_ONLY=0
NEW_VERSION=""

case "${1:-}" in
    --check) CHECK_ONLY=1 ;;
    -h|--help|"")
        sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) NEW_VERSION="$1" ;;
esac

if [ "$CHECK_ONLY" -eq 0 ]; then
    if ! printf '%s' "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "error: version must be MAJOR.MINOR.PATCH, got '$NEW_VERSION'" >&2
        exit 1
    fi
fi

# Read every declaration. Each entry is "label|current value".
read_versions() {
    printf 'package.json|%s\n'              "$(jq -r .version package.json)"
    printf 'package-lock.json (top)|%s\n'   "$(jq -r .version package-lock.json)"
    printf 'package-lock.json (pkgs)|%s\n'  "$(jq -r '.packages."".version' package-lock.json)"
    printf 'src-tauri/Cargo.toml|%s\n'      "$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)"
    printf 'src-tauri/tauri.conf.json|%s\n' "$(jq -r .version src-tauri/tauri.conf.json)"
    # Packaging manifests. A stale version here publishes a package whose
    # filename and contents disagree, and the AUR/COPR sources point at a git tag
    # that does not exist.
    printf 'packaging/aur/PKGBUILD|%s\n'    "$(sed -n 's/^pkgver=\(.*\)/\1/p' packaging/aur/PKGBUILD)"
    printf 'packaging/copr spec|%s\n'       "$(sed -n 's/^Version: *\(.*\)/\1/p' packaging/copr/thinkutils.spec)"
}

if [ "$CHECK_ONLY" -eq 1 ]; then
    expected=$(jq -r .version package.json)
    fail=0
    while IFS='|' read -r label value; do
        if [ "$value" = "$expected" ]; then
            printf '  ok   %-28s %s\n' "$label" "$value"
        else
            printf '  FAIL %-28s %s (expected %s)\n' "$label" "$value" "$expected"
            fail=1
        fi
    done < <(read_versions)

    if [ "$fail" -ne 0 ]; then
        echo
        echo "version declarations disagree - run ./scripts/bump-version.sh <version>" >&2
        exit 1
    fi
    echo
    echo "All version declarations agree at $expected"
    exit 0
fi

CURRENT=$(jq -r .version package.json)
echo "Bumping $CURRENT -> $NEW_VERSION"
echo

# Apply an edit and assert it actually changed the file. Silence here is the
# whole failure mode: a pattern that stops matching after a file is reformatted
# would leave that declaration stale with no indication anything went wrong.
edit() {
    local file="$1" expr="$2" label="$3"
    local before after
    before=$(cat "$file")
    after=$(printf '%s' "$before" | sed -E "$expr")

    if [ "$before" = "$after" ]; then
        echo "error: no change made to $file ($label)" >&2
        echo "       the pattern matched nothing - the file format may have changed" >&2
        exit 1
    fi
    printf '%s\n' "$after" > "$file"
    printf '  updated %s\n' "$label"
}

# package.json and tauri.conf.json: the first top-level "version" key.
edit package.json \
    "0,/\"version\": *\"[^\"]*\"/s//\"version\": \"$NEW_VERSION\"/" \
    "package.json"

edit src-tauri/tauri.conf.json \
    "0,/\"version\": *\"[^\"]*\"/s//\"version\": \"$NEW_VERSION\"/" \
    "src-tauri/tauri.conf.json"

# Cargo.toml: only the [package] version, never a dependency's.
edit src-tauri/Cargo.toml \
    "0,/^version = \"[^\"]*\"/s//version = \"$NEW_VERSION\"/" \
    "src-tauri/Cargo.toml"

# package-lock.json declares the version TWICE -- at the top level and under
# .packages."". Updating only the first is the classic half-bump, and npm ci
# will happily proceed with them disagreeing.
edit package-lock.json \
    "0,/\"version\": *\"[^\"]*\"/s//\"version\": \"$NEW_VERSION\"/" \
    "package-lock.json (top level)"

edit packaging/aur/PKGBUILD \
    "0,/^pkgver=.*/s//pkgver=$NEW_VERSION/" \
    "packaging/aur/PKGBUILD"

edit packaging/copr/thinkutils.spec \
    "0,/^Version: +.*/s//Version:        $NEW_VERSION/" \
    "packaging/copr/thinkutils.spec"

python3 - "$NEW_VERSION" <<'PY'
import json, sys
version = sys.argv[1]
with open('package-lock.json') as f:
    data = json.load(f)
if data.get('packages', {}).get('', {}).get('version') is None:
    raise SystemExit('error: package-lock.json has no .packages."".version to update')
data['packages']['']['version'] = version
with open('package-lock.json', 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
print('  updated package-lock.json (packages."")')
PY

# Cargo.lock records the crate's own version and is committed, so it has to be
# refreshed in the same commit or CI sees a dirty tree.
echo
echo "Refreshing Cargo.lock"
( cd src-tauri && cargo check --quiet 2>/dev/null ) || {
    echo "warning: cargo check failed - refresh Cargo.lock manually before committing" >&2
}

echo
"$0" --check

cat <<EOF

Next:
  git add -A && git commit -m "chore: bump to v$NEW_VERSION"
  git tag v$NEW_VERSION && git push && git push --tags

The tag must match the version above; release.yml rejects a mismatch after
the build has already run.
EOF
