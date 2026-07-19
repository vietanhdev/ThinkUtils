---
title: Download
description: Download ThinkUtils for Linux — .deb, .rpm and AppImage for ThinkPad laptops.
---

<script setup>
import { ref, onMounted, computed } from "vue";

const REPO = "vietanhdev/ThinkUtils";
const RELEASES_URL = `https://github.com/${REPO}/releases`;

const release = ref(null);
const failed = ref(false);

// Fetched at view time rather than baked in at build time: the docs site and the
// release pipeline deploy independently, so a hard-coded version here would go
// stale the moment a release ships without a docs rebuild. If GitHub is
// unreachable or rate-limits the request (60/hour per IP unauthenticated),
// `failed` flips and every button falls back to the releases page, which always
// works.
onMounted(async () => {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`);
    if (!res.ok) throw new Error(String(res.status));
    release.value = await res.json();
  } catch {
    failed.value = true;
  }
});

const version = computed(() => release.value?.tag_name ?? null);

// Matched by predicate rather than exact filename, so the page survives a version
// bump — the version is embedded in every asset name — without an edit here.
function find(pred) {
  return release.value?.assets?.find((a) => pred(a.name)) ?? null;
}

// Each predicate pins the architecture suffix. ThinkUtils ships x86_64 only
// today, but `find` returns the FIRST match, so a loose predicate would silently
// hand out the wrong package the moment a second architecture is added. The
// failure would be user-side and quiet: the page looks right, the download works,
// and the package refuses to install.
const is = {
  deb: (n) => n.endsWith("_amd64.deb"),
  rpm: (n) => n.endsWith(".x86_64.rpm"),
  appimage: (n) => n.endsWith("_amd64.AppImage"),
};

// Always yields a working link: the direct asset once a release exists, the
// releases page otherwise (no release cut yet, or the API call failed).
function url(pred) {
  return find(pred)?.browser_download_url ?? RELEASES_URL;
}
function size(pred) {
  const a = find(pred);
  return a ? `${(a.size / 1024 / 1024).toFixed(1)} MB` : "";
}
</script>

# Download ThinkUtils

<p class="dl-version">
  <template v-if="version">
    Latest release: <strong>{{ version }}</strong> · <a :href="RELEASES_URL">all releases</a>
  </template>
  <template v-else-if="failed">
    <a :href="RELEASES_URL">View all releases on GitHub →</a>
  </template>
  <template v-else>Looking up the latest release…</template>
</p>

For **Lenovo ThinkPad** laptops running Linux on **x86_64**. Built on Ubuntu 22.04
(glibc 2.35), so it runs on **Ubuntu 22.04+, Debian 12+ and Fedora 36+**.

Every release is installed into a clean container and launched under a virtual
display before it ships — on Ubuntu 22.04 and 24.04, Debian 12, and Fedora 41 —
with a screenshot checked by OCR to confirm the interface actually rendered.

<div class="dl-grid">
  <a class="dl-card" :href="url(is.deb)">
    <span class="dl-card-title">Debian / Ubuntu</span>
    <span class="dl-card-sub">.deb<template v-if="size(is.deb)"> · {{ size(is.deb) }}</template></span>
  </a>
  <a class="dl-card" :href="url(is.rpm)">
    <span class="dl-card-title">Fedora / RHEL</span>
    <span class="dl-card-sub">.rpm<template v-if="size(is.rpm)"> · {{ size(is.rpm) }}</template></span>
  </a>
  <a class="dl-card" :href="url(is.appimage)">
    <span class="dl-card-title">Any distro</span>
    <span class="dl-card-sub">AppImage<template v-if="size(is.appimage)"> · {{ size(is.appimage) }}</template></span>
  </a>
</div>

```bash
# Debian / Ubuntu
sudo apt install ./thinkutils_*_amd64.deb

# Fedora / RHEL
sudo dnf install ./thinkutils-*.x86_64.rpm

# AppImage — portable, nothing to install
chmod +x thinkutils_*_amd64.AppImage
./thinkutils_*_amd64.AppImage
```

::: tip Use `apt install ./file.deb`, not `dpkg -i`
`apt` pulls in the WebKit and GTK libraries ThinkUtils needs. `dpkg -i` does not,
and leaves you resolving them by hand.
:::

## Ubuntu APT repository

For automatic updates through `apt`:

```bash
echo "deb [trusted=yes] https://gh.vietanh.dev/ThinkUtils/apt ./" \
  | sudo tee /etc/apt/sources.list.d/thinkutils.list
sudo apt update
sudo apt install thinkutils
```

## Before fan control works

One step is not optional, and it is the most common reason people think the app
is broken. The `thinkpad_acpi` kernel module **refuses every fan change** unless
it was loaded with `fan_control=1`:

```bash
echo 'options thinkpad_acpi fan_control=1' \
  | sudo tee /etc/modprobe.d/thinkpad_acpi.conf
sudo modprobe -r thinkpad_acpi && sudo modprobe thinkpad_acpi
```

The app detects this and offers to do it for you on the Fan Control page. It is
worth knowing why granting permissions alone cannot fix it: the setting is a
kernel module parameter, fixed at load time, so no amount of privilege changes it
while the module is running.

Reboot if the reload fails — the module is often held open by something else.

::: warning Ubuntu 22.04 will still ask for your password
Ubuntu 22.04 ships polkit 0.105, which Debian and Ubuntu patched to ignore
JavaScript rule files. That is the mechanism ThinkUtils uses to grant passwordless
fan control, so on 22.04 every fan change prompts for a password. Everything
works; it is just not silent. Upgrading the distribution is the only fix.
:::

## Which ThinkPads are supported

Fan control needs the `thinkpad_acpi` kernel module, which covers most ThinkPads
from the X, T, P and L series. To check before installing:

```bash
ls /proc/acpi/ibm/fan && echo "supported"
```

Dual-fan machines — P1, P15, X1 Extreme and similar — are supported, and both
fans are reported. The firmware drives them together, so they cannot be set to
different speeds; that is a hardware limitation, not an app one.

Battery thresholds, CPU governor and system monitoring work on any Linux laptop.
Only fan control is ThinkPad-specific.

## Building from source

```bash
git clone https://github.com/vietanhdev/ThinkUtils.git
cd ThinkUtils
npm install
npm run tauri build
```

Packages land in `src-tauri/target/release/bundle/`. See the
[development guide](/development/architecture) for the toolchain you will need.
