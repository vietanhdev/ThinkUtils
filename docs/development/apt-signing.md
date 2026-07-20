# Signing the APT repository

The APT repository at `https://gh.vietanh.dev/ThinkUtils/apt` is published by the
`Update APT repository` step in `.github/workflows/release.yml`. That step signs
the `Release` file **if** a signing key is configured, and publishes unsigned
with a loud warning if one is not.

Until the key below exists, the repository is unsigned and users have to install
with `[trusted=yes]`.

## Why this matters here

`[trusted=yes]` tells apt to skip signature verification entirely. For most
repositories that is merely bad practice. For this one it is worse than average:

- the package installs a fan-control helper that runs as root, plus a polkit
  rule that grants access to it
- dpkg maintainer scripts run as root at install time

HTTPS authenticates the *host* for the duration of the transfer. It says nothing
about whether the bytes sitting on the `gh-pages` branch, or in any cache in
front of it, are the ones CI built. A signature is what carries that guarantee
from the builder to the user's machine.

## Generating the key

Do this once, on a trusted machine — not in CI.

```bash
# A dedicated key, used for nothing else.
gpg --quick-gen-key "ThinkUtils Archive Signing Key <you@example.com>" \
    default default never

# Note the key ID from the output, then export both halves.
KEYID=<key-id-from-above>
gpg --armor --export-secret-keys "$KEYID" > thinkutils-apt-private.asc
gpg --armor --export "$KEYID"             > thinkutils-apt-public.asc
```

Keep `thinkutils-apt-private.asc` offline. It is the credential that lets anyone
publish a package your users' machines will install as root.

## Configuring CI

Add two repository secrets (Settings → Secrets and variables → Actions):

| Secret                 | Value                                             |
| ---------------------- | ------------------------------------------------- |
| `APT_GPG_PRIVATE_KEY`  | full contents of `thinkutils-apt-private.asc`     |
| `APT_GPG_PASSPHRASE`   | the key's passphrase (omit if the key has none)   |

The next tagged release will then publish `InRelease`, `Release.gpg`, and
`thinkutils-archive-keyring.asc` alongside the packages, and the generated
`index.html` will switch to `signed-by=` instructions automatically.

## What users run once it is signed

```bash
curl -fsSL https://gh.vietanh.dev/ThinkUtils/apt/thinkutils-archive-keyring.asc \
  | sudo gpg --dearmor -o /usr/share/keyrings/thinkutils-archive-keyring.gpg

echo "deb [signed-by=/usr/share/keyrings/thinkutils-archive-keyring.gpg] https://gh.vietanh.dev/ThinkUtils/apt ./" \
  | sudo tee /etc/apt/sources.list.d/thinkutils.list

sudo apt update
sudo apt install thinkutils
```

`signed-by=` scopes the key to this one repository, so it cannot be used to
vouch for packages from anywhere else in the user's sources.

## Rotating or removing the key

Changing the key changes what users must trust, so treat it as a release event:
publish the new public key, and expect `apt update` to fail for anyone still
pinning the old one until they re-import.

Removing the secrets makes the next release publish unsigned again. The workflow
deletes any previously published `InRelease`, `Release.gpg`, and keyring file
when it does — a stale signature next to a regenerated `Release` breaks `apt
update` outright, which is worse for users than being unsigned.

## Verifying by hand

```bash
curl -fsSL https://gh.vietanh.dev/ThinkUtils/apt/thinkutils-archive-keyring.asc \
  | gpg --import
curl -fsSLO https://gh.vietanh.dev/ThinkUtils/apt/Release
curl -fsSLO https://gh.vietanh.dev/ThinkUtils/apt/Release.gpg
gpg --verify Release.gpg Release
```
