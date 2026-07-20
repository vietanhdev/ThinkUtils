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

// Releases now carry two architectures, which is exactly the case the previous
// comment here warned about: `find` returns the FIRST match, so a predicate that
// did not pin the suffix would hand an arm64 user an x86_64 package. That fails
// quietly on the user's side — the page looks right, the download works, and the
// package refuses to install.
//
// So the suffix stays pinned, and the arch becomes an explicit choice.
const ARCHES = {
  x86_64: { label: "Intel / AMD (x86_64)", deb: "amd64", rpm: "x86_64" },
  aarch64: { label: "ARM (aarch64)", deb: "arm64", rpm: "aarch64" },
};

// Guessed, never assumed. navigator.platform is deprecated and lies under some
// emulation layers, so it only picks the DEFAULT tab — both architectures stay
// one click away rather than being hidden behind a detection that can be wrong.
function detectArch() {
  const s = `${navigator.userAgent} ${navigator.platform ?? ""}`.toLowerCase();
  if (/aarch64|arm64/.test(s)) return "aarch64";
  return "x86_64";
}

const arch = ref("x86_64");
onMounted(() => {
  arch.value = detectArch();
});

const suffix = computed(() => ARCHES[arch.value]);

const is = {
  deb: (n) => n.endsWith(`_${suffix.value.deb}.deb`),
  rpm: (n) => n.endsWith(`.${suffix.value.rpm}.rpm`),
  appimage: (n) => n.endsWith(`_${suffix.value.deb}.AppImage`),
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

For **Lenovo ThinkPad** laptops running Linux on **x86_64 or ARM (aarch64)**.
Built on Ubuntu 22.04 (glibc 2.35), so it runs on **Ubuntu 22.04+, Debian 12+ and
Fedora 36+**.

Every release is installed into a clean container and launched under a virtual
display before it ships — on Ubuntu 22.04 and 24.04, Debian 12, and Fedora 41,
**on both architectures, on real hardware rather than emulation** — with a
screenshot checked by OCR to confirm the interface actually rendered.

<div class="dl-arch">
  <button
    v-for="(a, key) in ARCHES"
    :key="key"
    class="dl-arch-btn"
    :class="{ active: arch === key }"
    @click="arch = key"
  >{{ a.label }}</button>
</div>

::: warning Fan control is x86_64 only
On ARM ThinkPads (the X13s and similar) everything works **except fan control**.
That is a kernel limitation, not an app one: fan control goes through
`thinkpad_acpi`, which is an x86 platform driver and does not exist on ARM.

Battery charge thresholds, CPU governor, power profiles and system monitoring all
work normally — the app detects the missing fan interface and says so rather than
appearing broken.
:::

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
# Debian / Ubuntu — the glob matches whichever architecture you downloaded
sudo apt install ./thinkutils_*.deb

# Fedora / RHEL
sudo dnf install ./thinkutils-*.rpm

# AppImage — portable, nothing to install
chmod +x thinkutils_*.AppImage
./thinkutils_*.AppImage
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

::: warning What `[trusted=yes]` means
`[trusted=yes]` tells `apt` to install without verifying any signature. HTTPS
still authenticates the server for the duration of the download, but nothing
proves the packages are the ones our CI built — and ThinkUtils installs a helper
that runs as root.

It is needed because the **currently published** repository is unsigned. A
signing key is now configured, so the next release will publish a signed
repository and these instructions change to the ones below.

If you would rather not take that trade in the meantime, download the `.deb`
from the [releases page](https://github.com/vietanhdev/ThinkUtils/releases) and
install it with `apt install ./thinkutils_*.deb`. You give up automatic updates
and check for new versions yourself.
:::

### From the next release: verified installs

Once a signed release is published, `https://gh.vietanh.dev/ThinkUtils/apt` will
carry `InRelease`, `Release.gpg`, and the public key. Switch to:

```bash
curl -fsSL https://gh.vietanh.dev/ThinkUtils/apt/thinkutils-archive-keyring.asc \
  | sudo gpg --dearmor -o /usr/share/keyrings/thinkutils-archive-keyring.gpg

echo "deb [signed-by=/usr/share/keyrings/thinkutils-archive-keyring.gpg] https://gh.vietanh.dev/ThinkUtils/apt ./" \
  | sudo tee /etc/apt/sources.list.d/thinkutils.list

sudo apt update
sudo apt install thinkutils
```

`signed-by=` scopes the key to this one repository, so it cannot vouch for
packages from anywhere else in your sources. The repository's own index page
always shows whichever form is currently published — it is generated from what
the release actually produced, so it cannot disagree with reality.

Maintainers: see [apt-signing](/development/apt-signing).

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
