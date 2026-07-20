# Packaging

ThinkUtils ships to four channels. All of them install the same three things: the
binary, a package-owned fan helper, and a polkit rule scoped to that helper.

| Channel | Source | Helper path |
| --- | --- | --- |
| `.deb` / `.rpm` / AppImage | `npm run tauri build` | `/usr/local/bin` (self-installed) |
| AUR | `packaging/aur/PKGBUILD` | `/usr/lib/thinkutils/` |
| COPR | `packaging/copr/thinkutils.spec` | `%{_libexecdir}/thinkutils/` |
| PPA | `packaging/ppa/debian/` | `/usr/lib/thinkutils/` |

## Generated files

`packaging/helper/thinkutils-fan-control` and `packaging/polkit/50-thinkutils.rules`
are **generated from the Rust source**, not written by hand:

```bash
cd src-tauri && cargo run --example gen-packaging -- ../packaging
```

`src-tauri/tests/packaging.rs` fails if the committed copies drift. That matters
because the drift is silent: a polkit rule naming a path the helper is not
installed at grants nothing while looking correct, and the app falls back to a
password prompt that reads like a permissions problem.

## The PPA is the awkward one

Launchpad builders have **no network access**, so every Rust dependency is
vendored into the source tarball:

```bash
./scripts/build-ppa-source.sh --sign-key <KEYID>
dput ppa:vietanhng/thinkutils build/ppa/thinkutils_*_source.changes
```

Things worth knowing before touching it:

- **npm is not a build dependency.** `tauri.conf.json` has no `beforeBuildCommand`
  and `frontendDist` is a directory of static files. Vendoring a JS dependency
  tree deterministically is the hardest part of packaging a typical Tauri app,
  and this one avoids it entirely. Adding a bundler would change that.
- **The vendor tree is ~855 MB, ~77 MB compressed.** About three quarters is
  Windows-only crates. Deleting them looks like an easy win and **does not work** —
  cargo requires a vendored source for every crate in `Cargo.lock`, including
  platform-gated ones it never compiles. This was tested directly.
- **One `.orig` tarball per version**, shared by every series. Two runs producing
  byte-different tarballs for the same version get the second rejected. Hence the
  fixed mtime, sorted entries and single-threaded `xz` — `xz -T0` is not
  byte-reproducible.
- **`dh_clean -X.orig` is load-bearing.** Without it `dh_clean` deletes cargo's
  vendored `Cargo.toml.orig` files as patch cruft, and the offline build fails a
  checksum — on Launchpad only, since a local build never runs it.

### Series

`noble`, `resolute`, `stonking`.

`noble`'s default rustc is 1.75, which cannot parse a v4 `Cargo.lock`, so it
builds against the archive's versioned `rustc-1.83`/`cargo-1.83`.

**jammy (22.04) is deliberately excluded**, and not for toolchain reasons: it
ships polkit 0.105, whose JavaScript rules engine Debian and Ubuntu patched out.
The passwordless rule would be read by nothing there, so every fan change would
prompt for a password. Shipping a package whose headline feature silently
degrades is worse than not shipping it.

## Publishing

Three workflows, all `workflow_dispatch` only and all defaulting to `dry_run: true`:

| Workflow | Secret needed | Reversible? |
| --- | --- | --- |
| `publish-aur.yml` | `AUR_SSH_PRIVATE_KEY` | Yes — push again |
| `publish-copr.yml` | `COPR_API_TOKEN` | Yes — rebuild |
| `publish-ppa.yml` | `LAUNCHPAD_GPG_PRIVATE_KEY`, `LAUNCHPAD_GPG_PASSPHRASE` | **No** |

A Launchpad upload cannot be undone. A version can be superseded, never deleted,
which is why that workflow defaults to a dry run and prints every `.changes` file
before it would upload.

All three verify the git tag is **pushed** before doing anything: AUR's `source`
and COPR's `Source0` both point at the tag tarball, so a queued build would 404
partway through if the tag were only local.

### One-time setup

**AUR** — `ssh-keygen -t ed25519`, register the public half under My Account at
aur.archlinux.org, then `gh secret set AUR_SSH_PRIVATE_KEY < key`. Verify with
`ssh aur@aur.archlinux.org`; a welcome banner means it works.

**COPR** — copy the whole `[copr-cli]` block from
<https://copr.fedorainfracloud.org/api/> into `COPR_API_TOKEN`. Tokens expire; a
403 means regenerate. The workflow sets `--enable-net on` each run, because
`%build` fetches crates and mock disables builder networking by default — and
that setting lives on the *project*, so it does not travel with this repository.

**Launchpad** — the fiddliest:

1. Sign the Ubuntu Code of Conduct. Uploads are rejected without it.
2. `gpg --full-generate-key` (RSA 4096), dedicated to CI.
3. `gpg --send-keys --keyserver keyserver.ubuntu.com <KEYID>`.
4. Launchpad profile → OpenPGP keys → Import, paste the fingerprint. Launchpad
   emails an **encrypted** token; `gpg --decrypt` it and follow the link.
5. `gpg --armor --export-secret-keys <KEYID>` into `LAUNCHPAD_GPG_PRIVATE_KEY`.

The passphrase goes in `gpg.conf`, not a command-line argument, because
`dpkg-buildpackage` invokes `gpg` itself with no passphrase argument. The
gpg-agent preset approach does not work: `pinentry-mode loopback` bypasses the
agent entirely, so the preset is never consulted and signing fails headless with
"Operation cancelled".

## Version bumps

One command covers all seven declarations:

```bash
./scripts/bump-version.sh 0.1.11
```

Every edit asserts it changed something — a `sed` that silently matches nothing
is indistinguishable from success until release. CI runs `--check`.
