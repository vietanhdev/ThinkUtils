#!/usr/bin/env bash
#
# Launch-test the built packages in clean containers.
#
# Why this exists: CI proves the packages BUILD. Nothing has ever proved they
# RUN. "npm run tauri build exited 0" and "the .deb is 8MB" are both true of a
# binary that dies before it draws a window -- and this app builds a tray icon
# unconditionally against libayatana-appindicator3, which is exactly the
# dlopen-panic shape that has shipped broken in comparable projects.
#
# THE HARDWARE PROBLEM
# --------------------
# ThinkUtils reads /proc/acpi/ibm/fan and /sys/class/power_supply/BAT*. None of
# those exist in a container and none of them can. So this script does not try to
# test the hardware paths. It tests the thing that is currently untested and far
# more likely to break a release: that the app STARTS, RENDERS ITS FULL UI, and
# DEGRADES CLEANLY when the hardware is absent -- rather than crashing, hanging,
# or painting an empty window.
#
# It draws that distinction three ways, none of which touch hardware:
#
#  (a) The assertion surface is hardware-independent by construction.
#      src/index.html ships exactly two visible strings ("Home" and "Quick
#      settings and overview") plus four EMPTY containers. Every other word on
#      screen -- the sidebar labels, the section headings -- appears only because
#      templateLoader.js fetched a template over tauri:// and injected it. Those
#      labels are literal template markup; no /proc or /sys read produces them.
#      So OCR finding "Fan Control" proves the JS ran, on any machine. And OCR
#      finding "Home" while finding NO sidebar label is the exact signature of
#      "WebKit loaded the page, the JS died" -- checked explicitly below, because
#      every other assertion in this file passes in that state.
#
#  (b) The app declares its own hardware state. It prints "hw probe:" naming each
#      path it found and "hw mode:". In a container the correct, PASSING answer
#      is "degraded". An app claiming "full" in a container is detecting hardware
#      that is not there, which is reported as a warning.
#
#  (c) The frontend reports uncaught exceptions to the backend, which prints
#      them. This is the one that matters most: a view dying on an absent sysfs
#      path leaves the sidebar painted and the process alive, and that error line
#      is the only tell.
#
# Usage:
#   scripts/test-gui-packages-docker.sh                 # every target
#   scripts/test-gui-packages-docker.sh deb:debian:12   # one target
#   SETTLE=15 scripts/test-gui-packages-docker.sh
#
# Approach adapted from the sibling Bulwark project (Apache-2.0).

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="${ASSETS:-${REPO_ROOT}/src-tauri/target/release/bundle}"
OUTDIR="${OUTDIR:-${REPO_ROOT}/build/gui-test-out}"
READY_TIMEOUT="${READY_TIMEOUT:-60}"
SETTLE="${SETTLE:-5}"
STAGE="$(mktemp -d)"
trap 'rm -rf "${STAGE}"' EXIT

mkdir -p "${OUTDIR}"

# ThinkPads are x86_64. There is deliberately no arm64 matrix and no qemu:
# emulating a WebKit GUI is slow and fails on graphics paths no user touches, so
# a red result would be uninformative rather than useful.
ARCH="$(uname -m)"
if [ "${ARCH}" != "x86_64" ]; then
    echo "ERROR: this suite is x86_64-only (got ${ARCH})" >&2
    exit 1
fi

shopt -s nullglob

# The version currently declared, so the right artifact is picked out of a bundle
# directory that accumulates old builds. Resolving by glob alone would silently
# test whatever sorted first -- and `ls | head -1` sorts "0.1.10" before "0.1.5",
# so "newest" and "first" are not the same thing.
VERSION="$(jq -r .version "${REPO_ROOT}/package.json" 2>/dev/null || true)"
if [ -z "${VERSION}" ] || [ "${VERSION}" = "null" ]; then
    # Without this the version interpolates as empty and every glob becomes
    # something like thinkutils__amd64.deb, which matches nothing -- so the run
    # reports "no artifact" and the real cause (jq missing) stays hidden.
    echo "ERROR: could not read .version from package.json" >&2
    command -v jq >/dev/null || echo "       jq is not installed" >&2
    exit 1
fi

# Resolve the artifact for the CURRENT version. Never a hardcoded version (it
# would stop matching and silently test nothing) and never just "the only one"
# (stale builds accumulate locally, and a fresh CI checkout hides that).
stage_one() {
    local pattern="$1"
    local versioned="${pattern//VERSION/${VERSION}}"
    local matches=("${ASSETS}"/*/${versioned})

    if [ "${#matches[@]}" -eq 0 ]; then
        echo "ERROR: no artifact matching '${versioned}' under ${ASSETS}" >&2
        echo "       build first: npm run tauri build" >&2
        local any=("${ASSETS}"/*/${pattern//VERSION/*})
        if [ "${#any[@]}" -gt 0 ]; then
            echo "       found these other versions instead:" >&2
            printf '         %s\n' "${any[@]}" >&2
        fi
        return 1
    fi
    if [ "${#matches[@]}" -gt 1 ]; then
        echo "ERROR: ${#matches[@]} artifacts match '${versioned}' - cannot choose" >&2
        printf '  %s\n' "${matches[@]}" >&2
        return 1
    fi

    # Stale artifacts are not fatal here, but they are worth saying out loud:
    # release.yml selects with `ls *.deb | head -n 1`, which would rename an old
    # package to the new version's name and publish it.
    local all=("${ASSETS}"/*/${pattern//VERSION/*})
    if [ "${#all[@]}" -gt 1 ]; then
        echo "  note: ${#all[@]} builds present, testing v${VERSION}. Stale artifacts:" >&2
        for f in "${all[@]}"; do
            [ "$f" = "${matches[0]}" ] || echo "          $(basename "$f")" >&2
        done
    fi

    cp "${matches[0]}" "${STAGE}/"
    basename "${matches[0]}"
}

TARGETS=("$@")
if [ "${#TARGETS[@]}" -eq 0 ]; then
    TARGETS=(
        deb:ubuntu:22.04      # the build floor -- release.yml builds here, and it
                              # is the oldest release carrying libwebkit2gtk-4.1
        deb:ubuntu:24.04      # current LTS, where most users are
        deb:debian:12         # non-Ubuntu Debian: different WebKit and GTK point
                              # revisions, different appindicator packaging
        rpm:fedora:41         # the ONLY coverage the .rpm has ever had. It is
                              # bundled by Tauri running on Ubuntu, so its
                              # generated Requires: come from a Debian view of
                              # the world. Pinned, not :latest -- a Fedora rebase
                              # must not turn CI red for unrelated reasons.
        appimage:ubuntu:22.04 # an AppImage is a portability claim; test it on the
                              # oldest supported base where a missing bundled
                              # library actually surfaces
    )
fi

# Xvfb plus software rendering. Containers have no GPU, and WebKitGTK's DMA-BUF
# renderer fails without one. These are harness settings, not app requirements.
#
# xcompmgr is here for a ThinkUtils-specific reason: tauri.conf.json sets
# transparent:true and decorations:false. An ARGB window with no compositing
# manager has undefined content behind it under Xvfb, which makes the colour
# count and OCR read garbage. If it is unavailable the run continues and the
# pixel checks degrade to warnings rather than lying.
read -r -d '' GUI_ENV <<'ENVEOF' || true
  for t in Xvfb import identify compare xdotool pgrep tesseract convert; do
    command -v "$t" >/dev/null || {
      echo "HARNESS ERROR: $t missing - test dependencies failed to install."
      echo "This is a broken test environment, NOT an application failure."
      exit 90; }
  done
  export DISPLAY=:99
  export WEBKIT_DISABLE_COMPOSITING_MODE=1
  export WEBKIT_DISABLE_DMABUF_RENDERER=1
  export LIBGL_ALWAYS_SOFTWARE=1
  export GDK_BACKEND=x11
  export NO_AT_BRIDGE=1
  Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
  # Poll for the X socket rather than sleeping a guessed interval.
  for i in $(seq 1 30); do [ -e /tmp/.X11-unix/X99 ] && break; sleep 0.5; done
  [ -e /tmp/.X11-unix/X99 ] || { echo "HARNESS ERROR: Xvfb never came up"; exit 90; }
  command -v xcompmgr >/dev/null && { xcompmgr -a >/dev/null 2>&1 & sleep 1; }
  # Baseline of the EMPTY display, captured before launch. The colour count alone
  # cannot tell "the app painted" from "Xvfb has a noisy root"; this can, and it
  # stays valid if the UI is restyled.
  import -window root /tmp/baseline.png 2>/dev/null || true
ENVEOF

# Wait for the readiness marker instead of sleeping a guessed interval. A flat
# sleep is slower than needed on a fast runner and flaky on a slow one.
read -r -d '' WAIT_READY <<'WEOF' || true
  for i in $(seq 1 __READY_TIMEOUT__); do
    grep -q "\[thinkutils\] frontend ready:" /tmp/app.log 2>/dev/null && break
    kill -0 $APP_PID 2>/dev/null || break
    sleep 1
  done
  sleep __SETTLE__
WEOF
WAIT_READY="${WAIT_READY//__READY_TIMEOUT__/${READY_TIMEOUT}}"
WAIT_READY="${WAIT_READY//__SETTLE__/${SETTLE}}"

# Package installs are retried: mirror hiccups are the single most common cause
# of a red run that has nothing to do with the code.
read -r -d '' RETRY <<'REOF' || true
  retry() { for i in 1 2 3; do "$@" && return 0; echo "  (attempt $i failed, retrying)"; sleep 5; done; return 1; }
REOF

read -r -d '' VERDICT <<'VEOF' || true
  echo "----- app output -----"; cat /tmp/app.log; echo "----------------------"
  fail=0
  cp /tmp/app.log /out/app.log 2>/dev/null || true

  # -------------------------------------------------------------- crashes
  grep -qi "panicked" /tmp/app.log && { echo "FAIL: panicked on launch"; fail=1; }

  # Failures that neither panic nor kill the process: the app keeps running and
  # the user gets a broken window. The appindicator entry is not hypothetical --
  # this app builds a tray icon unconditionally.
  for sig in "undefined symbol" "cannot open shared object" "Segmentation fault" \
             "Failed to load module" "WebKitWebProcess.*crashed" "Exec format error"; do
    grep -qE "${sig}" /tmp/app.log && {
      echo "FAIL: log contains a known-bad signature: ${sig}"; fail=1; }
  done

  # ------------------------------------------------ must be a RELEASE build
  grep -qE "webview url: https?://" /tmp/app.log && {
    echo "FAIL: DEV build - the UI loads over http, not the embedded frontend"; fail=1; }
  grep -q "webview url: tauri://" /tmp/app.log || {
    echo "FAIL: webview did not load the embedded frontend"; fail=1; }

  # ----------------------------------------------------- hardware, honestly
  # The probe line existing at all proves the detection path RAN and returned
  # rather than panicking on a missing file.
  if grep -q "\[thinkutils\] hw probe:" /tmp/app.log; then
    echo "OK: $(grep -m1 'hw probe:' /tmp/app.log)"
  else
    echo "FAIL: no hardware probe line - detection never completed"; fail=1
  fi
  if grep -q "hw mode: degraded" /tmp/app.log; then
    echo "OK: correctly reports degraded mode (no ThinkPad hardware in a container)"
  elif grep -q "hw mode: full" /tmp/app.log; then
    echo "WARN: reports FULL hardware mode inside a container - detection is"
    echo "      producing false positives. Not failing this suite, but it is a bug."
  fi

  # ------------------------------------------- the frontend finished booting
  if grep -q "\[thinkutils\] frontend ready:" /tmp/app.log; then
    echo "OK: $(grep -m1 'frontend ready:' /tmp/app.log)"
  else
    echo "FAIL: frontend never signalled ready - JS init did not complete"; fail=1
  fi
  # The check that catches a view dying on a missing sysfs path while the sidebar
  # still paints and the process still lives.
  if grep -q "\[thinkutils\] frontend error:" /tmp/app.log; then
    echo "FAIL: uncaught frontend exception(s):"
    grep "frontend error:" /tmp/app.log | head -10 | sed "s/^/    /"
    fail=1
  fi

  kill -0 $APP_PID 2>/dev/null || { echo "FAIL: process died during settle"; fail=1; }

  # -------------------------------------------------- the web engine started
  if pgrep -f "WebKitWebProcess" >/dev/null 2>&1; then
    echo "OK: WebKitWebProcess is running"
  else
    echo "FAIL: no WebKitWebProcess - the webview never started"; fail=1
  fi

  # --------------------------------------------------------- a real window
  # Pick the LARGEST match, never the first: there is no window manager, and GTK
  # maps small helper windows carrying the same name.
  WIDS="$(xdotool search --name "ThinkUtils" 2>/dev/null || true)"
  # decorations:false means GTK, not a WM, owns the title. Fall back to all
  # visible windows so a naming change degrades rather than falsely failing.
  [ -z "${WIDS}" ] && WIDS="$(xdotool search --onlyvisible --name "." 2>/dev/null || true)"
  WID=""; BESTA=0; W=0; H=0
  for w in ${WIDS}; do
    GW="$(xdotool getwindowgeometry --shell "${w}" 2>/dev/null | sed -n "s/^WIDTH=//p")"
    GH="$(xdotool getwindowgeometry --shell "${w}" 2>/dev/null | sed -n "s/^HEIGHT=//p")"
    [ -n "${GW}" ] && [ -n "${GH}" ] || continue
    A=$(( GW * GH ))
    echo "  candidate window ${w}: ${GW}x${GH}"
    [ "${A}" -gt "${BESTA}" ] && { BESTA="${A}"; WID="${w}"; W="${GW}"; H="${GH}"; }
  done
  if [ -n "${WID}" ]; then
    echo "window: id=${WID} ${W}x${H}"
    # tauri.conf.json declares 1200x700 with minimums of 700x600. Assert well
    # below the minimum so a legitimate size change never breaks CI, but 0x0 does.
    if [ "${W}" -lt 600 ] || [ "${H}" -lt 400 ]; then
      echo "FAIL: largest window is only ${W}x${H} - too small to be usable"; fail=1; fi
  else
    echo "FAIL: no window is mapped on the display"; fail=1
  fi

  # ---------------------------------------------------------------- pixels
  # Capture the ROOT and crop rather than shooting the window: the window is ARGB
  # (transparent:true), and import -window on an ARGB drawable without a
  # compositor returns unreliable alpha. The root is always plain RGB.
  import -window root /tmp/shot_root.png 2>/dev/null || true
  if [ -n "${WID}" ] && [ -f /tmp/shot_root.png ]; then
    X="$(xdotool getwindowgeometry --shell "${WID}" | sed -n "s/^X=//p")"
    Y="$(xdotool getwindowgeometry --shell "${WID}" | sed -n "s/^Y=//p")"
    convert /tmp/shot_root.png -crop "${W}x${H}+${X:-0}+${Y:-0}" +repage /tmp/shot.png 2>/dev/null \
      || cp /tmp/shot_root.png /tmp/shot.png
  else
    cp /tmp/shot_root.png /tmp/shot.png 2>/dev/null || true
  fi
  cp /tmp/shot.png /out/shot.png 2>/dev/null || true
  cp /tmp/shot_root.png /out/shot_root.png 2>/dev/null || true

  if [ -f /tmp/shot.png ]; then
    colors=$(identify -format "%k" /tmp/shot.png 2>/dev/null || echo 0)
    echo "distinct colours on screen: ${colors}"
    # Dark theme (--bg-primary #1a1a1a on --text-primary #fff). Antialiased white
    # on near-black still yields hundreds of greys, so this threshold fails only
    # on genuinely empty output, never on a restyle.
    [ "${colors:-0}" -lt 40 ] && {
      echo "FAIL: window appears blank (${colors} colours)"; fail=1; }

    # The screen actually CHANGED versus the pre-launch capture. Compared
    # root-to-root on purpose: compare ERRORS on differing dimensions rather than
    # reporting a difference, and that error would read as "not zero" and pass.
    if [ -f /tmp/baseline.png ] && [ -f /tmp/shot_root.png ]; then
      RAW="$(compare -metric AE /tmp/baseline.png /tmp/shot_root.png null: 2>&1 || true)"
      # compare prints "1023970 (0.999969)" and uses scientific notation for large
      # counts. Take the first field and compare in awk, which handles 5.4e+10.
      DIFF="${RAW%% *}"
      echo "pixels changed vs the pre-launch display: ${RAW}"
      if echo "${DIFF}" | grep -qE "^[0-9]+([.][0-9]+)?([eE][+-]?[0-9]+)?$"; then
        awk -v d="${DIFF}" "BEGIN{exit !(d>0)}" \
          && echo "OK: ${DIFF} pixels changed after launch" \
          || { echo "FAIL: display is pixel-identical to before launch"; fail=1; }
      else
        echo "WARN: could not measure the pre/post difference (${RAW})"
      fi
    fi

    # ------------------------------------------------------------------ OCR
    # Preprocessing is mandatory here. This UI is white text on #1a1a1a, and
    # tesseract is trained on dark-on-light. So: greyscale, invert, upscale (small
    # UI text is below tesseract's comfortable x-height at 1280x800), sharpen.
    # Both the inverted and raw images are read and their output unioned, so a
    # future light theme still works without touching this script.
    convert /tmp/shot.png -colorspace Gray -negate -resize 200% -sharpen 0x1 \
      /tmp/ocr_in.png 2>/dev/null || true
    tesseract /tmp/ocr_in.png /tmp/ocr_inv --psm 6 >/dev/null 2>&1 || true
    tesseract /tmp/shot.png  /tmp/ocr_raw --psm 6 >/dev/null 2>&1 || true
    OCRTXT="$(cat /tmp/ocr_inv.txt /tmp/ocr_raw.txt 2>/dev/null || echo "")"
    printf '%s' "${OCRTXT}" > /out/ocr.txt 2>/dev/null || true
    echo "----- OCR -----"; echo "${OCRTXT}" | grep -v "^[[:space:]]*$" | head -30; echo "---------------"

    # WHY THESE STRINGS: every label below lives in src/templates/, and reaches
    # the screen only if templateLoader.js fetched it over tauri:// and injected
    # it. They are literal markup -- not one is produced by reading /proc or /sys.
    # So they prove the JS ran AND are hardware-independent, which is exactly what
    # makes this test meaningful in a container with no ThinkPad in it.
    #
    # Deliberately lenient about WHICH labels: OCR on a software-rendered
    # screenshot is not reliable enough to demand a specific string, and pinning
    # one would make every copy change a release blocker.
    # Includes the first-run permissions dialog. On a machine that has never been
    # set up -- which every container is -- the app correctly opens that dialog
    # over the main view, so the sidebar is not what OCR sees. The dialog lives in
    # templates/dialogs.html, so it is injected-template text too and proves the
    # same thing. The first run of this test failed on exactly that.
    LABELS="Fan Control|Battery|Performance|Monitor|System Info|Security|AI Integration|Sync|About|System Overview|Quick Settings|Power Profile|CPU Governor|Turbo Boost|Permissions Required|One-time setup|WHAT WILL BE CONFIGURED|Fan speed control|Battery charge thresholds"
    HITS="$(echo "${OCRTXT}" | grep -oiE "${LABELS}" | tr "[:upper:]" "[:lower:]" | sort -u)"
    NHITS="$(echo "${HITS}" | grep -c . || true)"
    echo "injected-template labels recognised (${NHITS}): $(echo ${HITS} | tr '\n' ' ')"

    if [ "${NHITS:-0}" -ge 3 ]; then
      echo "OK: the frontend fetched, injected and painted its templates"
    else
      # The specific, nameable failure: WebKit rendered index.html's own static
      # text but templateLoader never ran. Every other check above passes here.
      if echo "${OCRTXT}" | grep -qiE "quick settings and overview"; then
        echo "FAIL: only index.html's STATIC text is on screen - templateLoader.js"
        echo "      never injected the sidebar or views. WebKit loaded the page;"
        echo "      the JS did not run."
      else
        NALNUM="$(echo "${OCRTXT}" | tr -cd "[:alnum:]" | wc -c)"
        echo "FAIL: no injected UI text recognised (${NALNUM} alphanumeric chars)"
        echo "      see shot.png and ocr.txt in the uploaded artifacts"
      fi
      fail=1
    fi
  else
    echo "WARN: could not capture a screenshot; rendering not verified"
  fi

  # ------------------------------------------------------- clean shutdown
  # A crash on teardown is still a crash the user meets every time they close the
  # window, and nothing above would reach it. This app runs a fan-curve background
  # task and an optional MCP server, both with teardown paths worth exercising --
  # and the fan curve now restores the fan to auto on exit.
  kill -TERM $APP_PID 2>/dev/null || true
  for i in $(seq 1 20); do kill -0 $APP_PID 2>/dev/null || break; sleep 1; done
  if kill -0 $APP_PID 2>/dev/null; then
    echo "FAIL: did not exit within 20s of SIGTERM (hung on shutdown)"
    kill -KILL $APP_PID 2>/dev/null || true; fail=1
  else
    wait $APP_PID 2>/dev/null; rc=$?
    case "${rc}" in
      0|143) echo "OK: exited cleanly on SIGTERM (rc=${rc})" ;;
      139)   echo "FAIL: segfaulted during shutdown"; fail=1 ;;
      134)   echo "FAIL: aborted during shutdown"; fail=1 ;;
      *)     echo "NOTE: exited with rc=${rc} on SIGTERM" ;;
    esac
  fi
  grep -qi "panicked" /tmp/app.log && { echo "FAIL: panicked during shutdown"; fail=1; }
  cp /tmp/app.log /out/app.log 2>/dev/null || true

  [ $fail -eq 0 ] && echo "PASS" || true
  exit $fail
VEOF

overall=0
for spec in "${TARGETS[@]}"; do
    kind="${spec%%:*}"
    image="${spec#*:}"
    [ "${image}" = "${kind}" ] && image=""
    slug="$(echo "${spec}" | tr ':/' '__')"
    tout="${OUTDIR}/${slug}"
    mkdir -p "${tout}"

    echo
    echo "=============================================================="
    echo ">> GUI launch test: ${kind} on ${image:-<default>}"
    echo "=============================================================="

    case "${kind}" in
        deb)
            pkg="$(stage_one "thinkutils_VERSION_amd64.deb")" || { overall=1; continue; }
            docker run --rm -v "${STAGE}:/a:ro" -v "${tout}:/out" "${image:-ubuntu:24.04}" bash -c "
              set -u
              export DEBIAN_FRONTEND=noninteractive
              ${RETRY}
              retry apt-get update -qq >/dev/null
              retry apt-get install -y -qq xvfb imagemagick x11-apps xdotool xcompmgr procps tesseract-ocr >/dev/null 2>&1
              apt-get install -y -qq /a/${pkg} >/dev/null 2>&1 || { echo 'FAIL: apt install failed'; exit 1; }
              ${GUI_ENV}
              thinkutils >/tmp/app.log 2>&1 &
              APP_PID=\$!
              ${WAIT_READY}
              ${VERDICT}
            "
            ;;
        rpm)
            pkg="$(stage_one "thinkutils-VERSION-*.x86_64.rpm")" || { overall=1; continue; }
            docker run --rm -v "${STAGE}:/a:ro" -v "${tout}:/out" "${image:-fedora:41}" bash -c "
              set -u
              ${RETRY}
              retry dnf install -y -q xorg-x11-server-Xvfb ImageMagick xdotool xcompmgr procps-ng tesseract >/dev/null 2>&1
              dnf install -y -q /a/${pkg} >/dev/null 2>&1 || { echo 'FAIL: dnf install failed'; exit 1; }
              ${GUI_ENV}
              thinkutils >/tmp/app.log 2>&1 &
              APP_PID=\$!
              ${WAIT_READY}
              ${VERDICT}
            "
            ;;
        appimage)
            # --appimage-extract rather than a direct run: mounting an AppImage
            # needs FUSE, which a container does not have. Extraction exercises
            # the same payload.
            pkg="$(stage_one "thinkutils_VERSION_amd64.AppImage")" || { overall=1; continue; }
            docker run --rm -v "${STAGE}:/a:ro" -v "${tout}:/out" "${image:-ubuntu:22.04}" bash -c "
              set -u
              export DEBIAN_FRONTEND=noninteractive
              ${RETRY}
              retry apt-get update -qq >/dev/null
              retry apt-get install -y -qq xvfb imagemagick x11-apps xdotool xcompmgr procps tesseract-ocr >/dev/null 2>&1
              cd /tmp && cp /a/${pkg} app.AppImage && chmod +x app.AppImage
              ./app.AppImage --appimage-extract >/dev/null 2>&1 || { echo 'FAIL: AppImage extract failed'; exit 1; }
              ${GUI_ENV}
              ./squashfs-root/AppRun >/tmp/app.log 2>&1 &
              APP_PID=\$!
              ${WAIT_READY}
              ${VERDICT}
            "
            ;;
        *)
            echo "unknown target: ${kind}" >&2
            exit 2
            ;;
    esac

    rc=$?
    if [ "${rc}" -eq 0 ]; then
        echo ">> ${kind} on ${image:-default}: PASS"
    else
        echo ">> ${kind} on ${image:-default}: FAIL (rc=${rc}) - artifacts in ${tout}"
        overall=1
    fi
done

echo
echo "=============================================================="
[ "${overall}" -eq 0 ] && echo "ALL GUI PACKAGES PASSED" || echo "SOME GUI PACKAGES FAILED"
exit "${overall}"
