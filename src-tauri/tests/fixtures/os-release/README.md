# Real `/etc/os-release` fixtures

Captured verbatim from official container images, not written by hand:

```sh
docker run --rm --entrypoint cat <image> /etc/os-release
```

| Fixture | Image |
| --- | --- |
| `ubuntu-24.04` | `ubuntu:24.04` |
| `ubuntu-22.04` | `ubuntu:22.04` |
| `debian-12` | `debian:12` |
| `fedora-41` | `fedora:41` |
| `arch` | `archlinux:latest` |
| `opensuse-tumbleweed` | `opensuse/tumbleweed` |
| `rocky-9` | `rockylinux:9` |
| `linuxmint-21` | `linuxmintd/mint21-amd64` |

Capturing them beat writing them from memory. Three surprises the real files
settled:

- **Arch has no `ID_LIKE`**, and its `VERSION_ID` is a build date
  (`20260712.0.555161`), not a release number.
- **Mint 21 sets `ID_LIKE=ubuntu`** alone — not `"ubuntu debian"`.
- **Quoting is inconsistent across distros.** `arch` and `fedora` leave values
  bare; `opensuse-tumbleweed` and `rocky` quote them. A parser that only handles
  one form passes on half of these.

`.github/workflows/ci.yml` re-fetches these from the live images and fails if a
distro has drifted, so the fixtures cannot silently go stale.
