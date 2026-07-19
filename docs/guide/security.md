# Security

Built-in virus scanning powered by ClamAV.

![Security](/screenshots/security.png)

## Features

- **File/Directory Scanning**: Scan any file or directory for threats
- **Real-time Results**: View scan progress and detected threats
- **ClamAV Integration**: Uses the industry-standard open-source antivirus engine

## Prerequisites

ClamAV must be installed on your system:

::: code-group
```bash [Debian/Ubuntu]
sudo apt install clamav clamav-daemon
sudo freshclam  # Update virus definitions
```

```bash [Fedora]
sudo dnf install clamav clamav-update
sudo freshclam
```

```bash [Arch Linux]
sudo pacman -S clamav
sudo freshclam
```
:::
