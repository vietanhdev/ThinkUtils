#!/usr/bin/env bash
#
# Capture this machine's fan, battery, thermal and CPU interfaces as a test
# fixture, so CI can exercise real hardware shapes without real hardware.
#
# ThinkUtils reads /proc/acpi/ibm/fan and a spread of /sys paths that do not
# exist inside a container. Without fixtures, container tests can only prove the
# app starts -- they cannot prove it reads a dual-fan ThinkPad correctly, or that
# it degrades sanely on a machine with no ThinkPad fan at all.
#
# Contributing a profile is the main way to get a machine supported that the
# maintainers do not own. Run this, check the output, open a PR.
#
#   ./scripts/capture-hardware-profile.sh
#   ./scripts/capture-hardware-profile.sh --name my-t14-gen3
#
# Everything captured is world-readable and read-only. The script writes nothing
# outside its output directory and never touches the hardware.

set -euo pipefail

PROFILE_NAME=""
OUT_ROOT="src-tauri/tests/fixtures/hardware"

while [ $# -gt 0 ]; do
    case "$1" in
        --name) PROFILE_NAME="${2:-}"; shift 2 ;;
        --out) OUT_ROOT="${2:-}"; shift 2 ;;
        -h|--help)
            sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done

# Derive a profile name from the DMI model when not given one. product_version
# carries the marketing name on ThinkPads ("ThinkPad P1 Gen 4i"); product_name
# is the machine type ("20Y4S2ND00").
if [ -z "$PROFILE_NAME" ]; then
    model=$(cat /sys/class/dmi/id/product_version 2>/dev/null || echo "unknown")
    PROFILE_NAME=$(printf '%s' "$model" \
        | tr '[:upper:]' '[:lower:]' \
        | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')
    [ -n "$PROFILE_NAME" ] || PROFILE_NAME="unknown"
fi

DEST="$OUT_ROOT/$PROFILE_NAME"
mkdir -p "$DEST"

echo "Capturing hardware profile: $PROFILE_NAME"
echo "  -> $DEST"

# --- Values that identify the individual machine, not the model ---
# Serial numbers, UUIDs and asset tags say nothing useful about hardware shape
# and must not land in a public repo.
#
# Filtering by filename alone is not enough: `uevent` is an aggregate file whose
# name says nothing, and it carries POWER_SUPPLY_SERIAL_NUMBER inside. So content
# is scrubbed too, line by line, on the way in.
REDACT_PATTERN='serial|uuid|asset|_id$|address'
REDACT_CONTENT_PATTERN='SERIAL|UUID|ASSET|MAC_ADDRESS'

# Copy a file into the fixture tree, preserving its path so tests can point a
# root at this directory and have every lookup resolve unchanged.
capture() {
    local src="$1"
    [ -r "$src" ] || return 0
    local dst="$DEST${src}"
    mkdir -p "$(dirname "$dst")"
    # Reads can fail on sysfs attributes that are write-only or that error on
    # read (some EC attributes do). Skip rather than abort the whole capture.
    if ! cat "$src" > "$dst.tmp" 2>/dev/null; then
        rm -f "$dst.tmp"
        return 0
    fi
    # Replace the value of any identifying key, keeping the key so the file
    # shape stays realistic for parsing tests.
    sed -E "s/^([A-Za-z_]*($REDACT_CONTENT_PATTERN)[A-Za-z_]*)=.*/\1=REDACTED/" \
        "$dst.tmp" > "$dst"
    rm -f "$dst.tmp"
}

capture_dir() {
    local dir="$1"
    [ -d "$dir" ] || return 0
    for f in "$dir"/*; do
        [ -f "$f" ] || continue
        base=$(basename "$f")
        if printf '%s' "$base" | grep -qiE "$REDACT_PATTERN"; then
            continue
        fi
        capture "$f"
    done
}

# --- ThinkPad fan (the control interface) ---
capture /proc/acpi/ibm/fan

# --- hwmon: the portable read path, and where a second fan shows up ---
for h in /sys/class/hwmon/hwmon*; do
    [ -e "$h/name" ] || continue
    # Resolve through the symlink so the fixture holds a real directory tree.
    real=$(readlink -f "$h")
    capture "$real/name"
    for attr in "$real"/fan*_input "$real"/fan*_label \
                "$real"/pwm[0-9] "$real"/pwm[0-9]_enable \
                "$real"/temp*_input "$real"/temp*_label "$real"/temp*_crit "$real"/temp*_max; do
        [ -e "$attr" ] && capture "$attr"
    done
    # Mirror the /sys/class/hwmon view as real directories.
    #
    # On a live system these are symlinks into /sys/devices, and capturing only
    # the resolved target leaves a profile that nothing can discover: code walks
    # /sys/class/hwmon, which would not exist. Symlinks are not used here either,
    # since they would point outside the profile at the real machine.
    class_dir="$DEST/sys/class/hwmon/$(basename "$h")"
    mkdir -p "$class_dir"
    for f in "$real"/*; do
        [ -f "$f" ] || continue
        base=$(basename "$f")
        case "$base" in
            name|fan*|pwm*|temp*) cat "$f" > "$class_dir/$base" 2>/dev/null || true ;;
        esac
    done

    # Record the mapping too, so the provenance of each chip stays readable.
    printf '%s -> %s\n' "$h" "$real" >> "$DEST/hwmon-map.txt"
done

# --- Batteries and AC ---
for ps in /sys/class/power_supply/*; do
    [ -e "$ps/type" ] || continue
    capture_dir "$(readlink -f "$ps")"
done

# --- CPU frequency scaling and turbo ---
capture_dir /sys/devices/system/cpu/cpu0/cpufreq
capture /sys/devices/system/cpu/intel_pstate/no_turbo
capture /sys/devices/system/cpu/intel_pstate/status
capture /sys/devices/system/cpu/amd_pstate/status

# --- Thermal zones ---
for tz in /sys/class/thermal/thermal_zone*; do
    [ -e "$tz/type" ] || continue
    capture "$(readlink -f "$tz")/type"
    capture "$(readlink -f "$tz")/temp"
done

# --- Model identification, minus anything machine-unique ---
{
    printf 'product_version=%s\n' "$(cat /sys/class/dmi/id/product_version 2>/dev/null || echo unknown)"
    printf 'product_name=%s\n'    "$(cat /sys/class/dmi/id/product_name 2>/dev/null || echo unknown)"
    printf 'sys_vendor=%s\n'      "$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || echo unknown)"
    printf 'kernel=%s\n'          "$(uname -r)"
} > "$DEST/machine.txt"

# --- A summary that makes the profile readable at a glance in review ---
{
    echo "# Hardware profile: $PROFILE_NAME"
    echo
    sed 's/^/    /' "$DEST/machine.txt"
    echo
    if [ -r /proc/acpi/ibm/fan ]; then
        fan_cmds=$(grep -c '^commands:' /proc/acpi/ibm/fan 2>/dev/null || echo 0)
        echo "ThinkPad fan interface: present"
        echo "fan_control=1 active:   $([ "$fan_cmds" -gt 0 ] && echo yes || echo 'no (writes would be refused)')"
    else
        echo "ThinkPad fan interface: absent"
    fi
    tach_count=$(find "$DEST/sys" -name 'fan[0-9]_input' 2>/dev/null | wc -l)
    pwm_count=$(find "$DEST/sys" -name 'pwm[0-9]' 2>/dev/null | wc -l)
    echo "Tachometers captured:   $tach_count"
    echo "PWM channels captured:  $pwm_count"
} > "$DEST/README.md"

echo
cat "$DEST/README.md"
echo
echo "Review the files before committing -- anything that identifies your"
echo "individual machine (rather than its model) should not be included."
