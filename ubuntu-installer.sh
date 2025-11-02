#!/bin/bash
#
# ThinkUtils - Ubuntu Package Installer
# Interactive script for installing common packages on Ubuntu
#
# Usage: ./ubuntu-installer.sh [--dry-run] [--profile <file>]
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$HOME/.config/thinkutils"
PROFILE_DIR="$CONFIG_DIR/profiles"
LOG_FILE="$CONFIG_DIR/install.log"
DRY_RUN=false

# Package definitions
declare -A PACKAGE_GROUPS
declare -A PACKAGE_DESCRIPTIONS

# ThinkPad Utilities
PACKAGE_GROUPS[thinkpad]="tlp tlp-rdw acpi-call-dkms powertop lm-sensors"
PACKAGE_DESCRIPTIONS[thinkpad]="ThinkPad power management and monitoring tools"

# Development Tools
PACKAGE_GROUPS[development]="build-essential git curl wget vim neovim gcc g++ make cmake"
PACKAGE_DESCRIPTIONS[development]="Essential development tools and compilers"

# System Monitoring
PACKAGE_GROUPS[monitoring]="htop btop neofetch lm-sensors sysstat iotop"
PACKAGE_DESCRIPTIONS[monitoring]="System monitoring and performance tools"

# Productivity
PACKAGE_GROUPS[productivity]="tmux screen zsh fish terminator tilix"
PACKAGE_DESCRIPTIONS[productivity]="Terminal multiplexers and shells"

# Media Tools
PACKAGE_GROUPS[media]="vlc ffmpeg imagemagick gimp inkscape"
PACKAGE_DESCRIPTIONS[media]="Media players and editing tools"

# Networking
PACKAGE_GROUPS[networking]="net-tools nmap wireshark traceroute iperf3"
PACKAGE_DESCRIPTIONS[networking]="Network diagnostic and monitoring tools"

# Security
PACKAGE_GROUPS[security]="ufw fail2ban clamav rkhunter"
PACKAGE_DESCRIPTIONS[security]="Security and firewall tools"

# Python Development
PACKAGE_GROUPS[python]="python3 python3-pip python3-venv python3-dev ipython3"
PACKAGE_DESCRIPTIONS[python]="Python development environment"

# Rust Development
PACKAGE_GROUPS[rust]="cargo rustc"
PACKAGE_DESCRIPTIONS[rust]="Rust programming language"

# Node.js Development
PACKAGE_GROUPS[nodejs]="nodejs npm"
PACKAGE_DESCRIPTIONS[nodejs]="Node.js and npm package manager"

# Functions

print_header() {
    echo -e "${BOLD}${CYAN}"
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║                                                            ║"
    echo "║              ThinkUtils - Ubuntu Installer                 ║"
    echo "║                                                            ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_section() {
    echo -e "\n${BOLD}${BLUE}▶ $1${NC}"
    echo -e "${BLUE}$(printf '─%.0s' {1..60})${NC}"
}

log_message() {
    local level=$1
    shift
    local message="$@"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" >> "$LOG_FILE"
    
    case $level in
        INFO)
            echo -e "${CYAN}ℹ${NC} $message"
            ;;
        SUCCESS)
            echo -e "${GREEN}✓${NC} $message"
            ;;
        WARNING)
            echo -e "${YELLOW}⚠${NC} $message"
            ;;
        ERROR)
            echo -e "${RED}✗${NC} $message"
            ;;
    esac
}

check_requirements() {
    print_section "Checking Requirements"
    
    # Check if running on Ubuntu
    if [ ! -f /etc/os-release ]; then
        log_message ERROR "Cannot detect OS. This script is for Ubuntu only."
        exit 1
    fi
    
    source /etc/os-release
    if [[ "$ID" != "ubuntu" ]]; then
        log_message ERROR "This script is designed for Ubuntu. Detected: $ID"
        exit 1
    fi
    
    log_message SUCCESS "Running on Ubuntu $VERSION"
    
    # Check if dialog is installed
    if ! command -v dialog &> /dev/null; then
        log_message WARNING "dialog not found. Installing..."
        sudo apt-get update -qq
        sudo apt-get install -y dialog
    fi
    
    # Check internet connection
    if ! ping -c 1 8.8.8.8 &> /dev/null; then
        log_message ERROR "No internet connection detected"
        exit 1
    fi
    
    log_message SUCCESS "Internet connection OK"
    
    # Create config directory
    mkdir -p "$CONFIG_DIR" "$PROFILE_DIR"
}

show_package_menu() {
    local temp_file=$(mktemp)
    local options=()
    local index=1
    
    # Build menu options
    for group in "${!PACKAGE_GROUPS[@]}"; do
        options+=("$index" "${group^}: ${PACKAGE_DESCRIPTIONS[$group]}" "off")
        ((index++))
    done
    
    # Add custom packages option
    options+=("$index" "Custom: Enter package names manually" "off")
    
    dialog --backtitle "ThinkUtils Installer" \
           --title "Package Selection" \
           --checklist "Select package groups to install (Space to select, Enter to confirm):" \
           20 80 12 \
           "${options[@]}" \
           2>"$temp_file"
    
    local exit_code=$?
    local selections=$(cat "$temp_file")
    rm "$temp_file"
    
    if [ $exit_code -ne 0 ]; then
        log_message INFO "Installation cancelled by user"
        exit 0
    fi
    
    echo "$selections"
}

get_packages_from_selections() {
    local selections="$1"
    local packages=""
    local index=1
    
    for group in "${!PACKAGE_GROUPS[@]}"; do
        if echo "$selections" | grep -q "\"$index\""; then
            packages="$packages ${PACKAGE_GROUPS[$group]}"
        fi
        ((index++))
    done
    
    # Check if custom packages option was selected
    if echo "$selections" | grep -q "\"$index\""; then
        local custom_packages=$(dialog --backtitle "ThinkUtils Installer" \
                                       --title "Custom Packages" \
                                       --inputbox "Enter package names (space-separated):" \
                                       10 60 \
                                       3>&1 1>&2 2>&3)
        packages="$packages $custom_packages"
    fi
    
    echo "$packages" | xargs
}

check_installed_packages() {
    local packages="$1"
    local to_install=""
    local already_installed=""
    
    for pkg in $packages; do
        if dpkg -l | grep -q "^ii  $pkg "; then
            already_installed="$already_installed $pkg"
        else
            to_install="$to_install $pkg"
        fi
    done
    
    echo "$to_install|$already_installed"
}

calculate_disk_space() {
    local packages="$1"
    
    if [ -z "$packages" ]; then
        echo "0"
        return
    fi
    
    # Simulate installation to get size
    local size=$(apt-get -s install $packages 2>/dev/null | \
                 grep "^Need to get" | \
                 awk '{print $4}' | \
                 sed 's/[^0-9.]//g')
    
    echo "${size:-0}"
}

show_installation_summary() {
    local packages="$1"
    local already_installed="$2"
    local disk_space="$3"
    
    local temp_file=$(mktemp)
    
    {
        echo "Installation Summary"
        echo "===================="
        echo ""
        echo "Packages to install:"
        for pkg in $packages; do
            echo "  • $pkg"
        done
        echo ""
        
        if [ -n "$already_installed" ]; then
            echo "Already installed (will be skipped):"
            for pkg in $already_installed; do
                echo "  • $pkg"
            done
            echo ""
        fi
        
        echo "Estimated download size: ${disk_space} MB"
        echo ""
        echo "Proceed with installation?"
    } > "$temp_file"
    
    dialog --backtitle "ThinkUtils Installer" \
           --title "Confirmation" \
           --yesno "$(cat $temp_file)" \
           20 70
    
    local result=$?
    rm "$temp_file"
    return $result
}

install_packages() {
    local packages="$1"
    
    if [ -z "$packages" ]; then
        log_message WARNING "No packages to install"
        return 0
    fi
    
    print_section "Installing Packages"
    
    if [ "$DRY_RUN" = true ]; then
        log_message INFO "DRY RUN: Would install: $packages"
        return 0
    fi
    
    # Update package list
    log_message INFO "Updating package list..."
    sudo apt-get update -qq || {
        log_message ERROR "Failed to update package list"
        return 1
    }
    
    # Install packages
    log_message INFO "Installing packages..."
    
    local failed_packages=""
    local success_count=0
    local total_count=$(echo $packages | wc -w)
    
    for pkg in $packages; do
        echo -ne "${CYAN}Installing $pkg...${NC}\r"
        
        if sudo apt-get install -y "$pkg" >> "$LOG_FILE" 2>&1; then
            log_message SUCCESS "Installed $pkg"
            ((success_count++))
        else
            log_message ERROR "Failed to install $pkg"
            failed_packages="$failed_packages $pkg"
        fi
    done
    
    echo ""
    log_message SUCCESS "Installed $success_count/$total_count packages"
    
    if [ -n "$failed_packages" ]; then
        log_message WARNING "Failed packages:$failed_packages"
        return 1
    fi
    
    return 0
}

configure_thinkpad_tools() {
    print_section "Configuring ThinkPad Tools"
    
    # Check if TLP was installed
    if dpkg -l | grep -q "^ii  tlp "; then
        log_message INFO "Configuring TLP for ThinkPad..."
        
        # Enable TLP
        sudo systemctl enable tlp.service
        sudo systemctl start tlp.service
        
        # Configure battery thresholds (if supported)
        if [ -f /sys/class/power_supply/BAT0/charge_control_start_threshold ]; then
            log_message INFO "Setting battery charge thresholds..."
            echo 40 | sudo tee /sys/class/power_supply/BAT0/charge_control_start_threshold > /dev/null
            echo 80 | sudo tee /sys/class/power_supply/BAT0/charge_control_end_threshold > /dev/null
            log_message SUCCESS "Battery thresholds set (40%-80%)"
        fi
        
        log_message SUCCESS "TLP configured"
    fi
    
    # Check if lm-sensors was installed
    if dpkg -l | grep -q "^ii  lm-sensors "; then
        log_message INFO "Detecting sensors..."
        sudo sensors-detect --auto >> "$LOG_FILE" 2>&1
        log_message SUCCESS "Sensors configured"
    fi
    
    # Check if thinkpad_acpi module is loaded
    if lsmod | grep -q thinkpad_acpi; then
        log_message SUCCESS "thinkpad_acpi module loaded"
        
        # Check if fan control is enabled
        if [ -f /proc/acpi/ibm/fan ]; then
            log_message SUCCESS "Fan control interface available"
        else
            log_message WARNING "Fan control not available. Enable with: options thinkpad_acpi fan_control=1"
        fi
    fi
}

save_profile() {
    local packages="$1"
    local profile_name=$(dialog --backtitle "ThinkUtils Installer" \
                                --title "Save Profile" \
                                --inputbox "Enter profile name:" \
                                10 50 \
                                3>&1 1>&2 2>&3)
    
    if [ -n "$profile_name" ]; then
        local profile_file="$PROFILE_DIR/${profile_name}.profile"
        echo "$packages" > "$profile_file"
        log_message SUCCESS "Profile saved: $profile_file"
    fi
}

load_profile() {
    local profiles=($(ls -1 "$PROFILE_DIR"/*.profile 2>/dev/null | xargs -n1 basename))
    
    if [ ${#profiles[@]} -eq 0 ]; then
        log_message WARNING "No saved profiles found"
        return 1
    fi
    
    local options=()
    local index=1
    
    for profile in "${profiles[@]}"; do
        options+=("$index" "${profile%.profile}")
        ((index++))
    done
    
    local selection=$(dialog --backtitle "ThinkUtils Installer" \
                             --title "Load Profile" \
                             --menu "Select a profile:" \
                             15 50 8 \
                             "${options[@]}" \
                             3>&1 1>&2 2>&3)
    
    if [ -n "$selection" ]; then
        local profile_file="$PROFILE_DIR/${profiles[$((selection-1))]}"
        cat "$profile_file"
    fi
}

show_final_summary() {
    local success=$1
    
    print_section "Installation Complete"
    
    if [ $success -eq 0 ]; then
        log_message SUCCESS "All packages installed successfully!"
        echo ""
        echo -e "${GREEN}${BOLD}Next Steps:${NC}"
        echo -e "  1. Restart your system to apply all changes"
        echo -e "  2. Run 'tlp-stat' to check TLP status"
        echo -e "  3. Run 'sensors' to view temperature sensors"
        echo -e "  4. Launch ThinkUtils for fan control"
    else
        log_message WARNING "Installation completed with some errors"
        echo -e "\n${YELLOW}Check the log file for details: $LOG_FILE${NC}"
    fi
    
    echo ""
    echo -e "${CYAN}Log file: $LOG_FILE${NC}"
}

main() {
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --profile)
                PROFILE_FILE="$2"
                shift 2
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --dry-run       Show what would be installed without installing"
                echo "  --profile FILE  Load packages from profile file"
                echo "  --help          Show this help message"
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    # Initialize
    print_header
    check_requirements
    
    # Get package selections
    local packages=""
    
    if [ -n "$PROFILE_FILE" ]; then
        packages=$(cat "$PROFILE_FILE")
        log_message INFO "Loaded profile: $PROFILE_FILE"
    else
        # Show main menu
        local action=$(dialog --backtitle "ThinkUtils Installer" \
                              --title "Main Menu" \
                              --menu "Choose an action:" \
                              15 60 4 \
                              1 "Install packages (interactive)" \
                              2 "Load saved profile" \
                              3 "Exit" \
                              3>&1 1>&2 2>&3)
        
        case $action in
            1)
                local selections=$(show_package_menu)
                packages=$(get_packages_from_selections "$selections")
                ;;
            2)
                packages=$(load_profile)
                ;;
            *)
                log_message INFO "Exiting"
                exit 0
                ;;
        esac
    fi
    
    if [ -z "$packages" ]; then
        log_message WARNING "No packages selected"
        exit 0
    fi
    
    # Check which packages are already installed
    local check_result=$(check_installed_packages "$packages")
    local to_install=$(echo "$check_result" | cut -d'|' -f1)
    local already_installed=$(echo "$check_result" | cut -d'|' -f2)
    
    # Calculate disk space
    local disk_space=$(calculate_disk_space "$to_install")
    
    # Show summary and confirm
    if ! show_installation_summary "$to_install" "$already_installed" "$disk_space"; then
        log_message INFO "Installation cancelled"
        exit 0
    fi
    
    # Install packages
    install_packages "$to_install"
    local install_result=$?
    
    # Configure ThinkPad-specific tools
    if [ $install_result -eq 0 ]; then
        configure_thinkpad_tools
    fi
    
    # Offer to save profile
    if [ "$DRY_RUN" = false ] && [ -z "$PROFILE_FILE" ]; then
        if dialog --backtitle "ThinkUtils Installer" \
                  --title "Save Profile" \
                  --yesno "Would you like to save this installation as a profile?" \
                  7 60; then
            save_profile "$packages"
        fi
    fi
    
    # Show final summary
    show_final_summary $install_result
    
    exit $install_result
}

# Run main function
main "$@"
