#!/bin/bash

# ThinkUtils Polkit Policy Installation Script
# This script installs the polkit policy to allow ThinkUtils to manage system settings

set -e

POLICY_FILE="polkit/com.thinkutils.policy"
INSTALL_DIR="/usr/share/polkit-1/actions"

echo "ThinkUtils Polkit Policy Installer"
echo "==================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo "This script must be run as root (use sudo)"
    exit 1
fi

# Check if policy file exists
if [ ! -f "$POLICY_FILE" ]; then
    echo "Error: Policy file not found at $POLICY_FILE"
    exit 1
fi

# Check if polkit is installed
if [ ! -d "$INSTALL_DIR" ]; then
    echo "Error: Polkit not found. Please install polkit first:"
    echo "  Ubuntu/Debian: sudo apt install polkit"
    echo "  Fedora: sudo dnf install polkit"
    echo "  Arch: sudo pacman -S polkit"
    exit 1
fi

# Install policy file
echo "Installing polkit policy..."
cp "$POLICY_FILE" "$INSTALL_DIR/"
chmod 644 "$INSTALL_DIR/com.thinkutils.policy"

echo ""
echo "✓ Polkit policy installed successfully!"
echo ""
echo "ThinkUtils can now:"
echo "  - Change CPU governor"
echo "  - Toggle turbo boost"
echo "  - Control fan speeds"
echo "  - Set battery charge thresholds"
echo ""
echo "You will be prompted for your password when using these features."
echo "The 'auth_admin_keep' policy means you won't need to re-authenticate"
echo "for a few minutes after the first authentication."
echo ""
