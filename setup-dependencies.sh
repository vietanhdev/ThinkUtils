#!/bin/bash
#
# ThinkUtils - Dependency Setup Script
# Checks and installs required dependencies
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                            ║${NC}"
echo -e "${BLUE}║              ThinkUtils - Dependency Setup                 ║${NC}"
echo -e "${BLUE}║                                                            ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
else
    echo -e "${RED}✗ Cannot detect OS${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Detected OS: $PRETTY_NAME${NC}"
echo ""

# Check for required commands
echo -e "${BLUE}Checking dependencies...${NC}"
echo ""

MISSING_DEPS=()

# Check lm-sensors
if ! command -v sensors &> /dev/null; then
    echo -e "${YELLOW}⚠ lm-sensors not found${NC}"
    MISSING_DEPS+=("lm-sensors")
else
    echo -e "${GREEN}✓ lm-sensors installed${NC}"
fi

# Check pkexec (polkit)
if ! command -v pkexec &> /dev/null; then
    echo -e "${YELLOW}⚠ pkexec (polkit) not found${NC}"
    case $OS in
        ubuntu|debian)
            MISSING_DEPS+=("policykit-1")
            ;;
        fedora|rhel|centos)
            MISSING_DEPS+=("polkit")
            ;;
        arch|manjaro)
            MISSING_DEPS+=("polkit")
            ;;
    esac
else
    echo -e "${GREEN}✓ pkexec (polkit) installed${NC}"
fi

# Check thinkpad_acpi module
if lsmod | grep -q thinkpad_acpi; then
    echo -e "${GREEN}✓ thinkpad_acpi module loaded${NC}"
    
    # Check if fan control is available
    if [ -f /proc/acpi/ibm/fan ]; then
        echo -e "${GREEN}✓ Fan control interface available${NC}"
        
        # Check if fan control is enabled
        if grep -q "level:" /proc/acpi/ibm/fan; then
            echo -e "${GREEN}✓ Fan control enabled${NC}"
        else
            echo -e "${YELLOW}⚠ Fan control may not be enabled${NC}"
            echo -e "  Run: echo 'options thinkpad_acpi fan_control=1' | sudo tee /etc/modprobe.d/thinkpad_acpi.conf"
        fi
    else
        echo -e "${YELLOW}⚠ Fan control interface not found${NC}"
    fi
else
    echo -e "${YELLOW}⚠ thinkpad_acpi module not loaded${NC}"
    echo -e "  This is normal if you're not on a ThinkPad"
fi

echo ""

# Install missing dependencies
if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo -e "${YELLOW}Missing dependencies: ${MISSING_DEPS[*]}${NC}"
    echo ""
    
    read -p "Install missing dependencies? (y/n) " -n 1 -r
    echo ""
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        case $OS in
            ubuntu|debian)
                echo -e "${BLUE}Installing with apt...${NC}"
                sudo apt update
                sudo apt install -y "${MISSING_DEPS[@]}"
                ;;
            fedora|rhel|centos)
                echo -e "${BLUE}Installing with dnf...${NC}"
                sudo dnf install -y "${MISSING_DEPS[@]}"
                ;;
            arch|manjaro)
                echo -e "${BLUE}Installing with pacman...${NC}"
                sudo pacman -S --noconfirm "${MISSING_DEPS[@]}"
                ;;
            *)
                echo -e "${RED}✗ Unsupported OS: $OS${NC}"
                echo "Please install manually: ${MISSING_DEPS[*]}"
                exit 1
                ;;
        esac
        
        echo ""
        echo -e "${GREEN}✓ Dependencies installed${NC}"
    fi
else
    echo -e "${GREEN}✓ All dependencies installed${NC}"
fi

echo ""

# Configure sensors if just installed
if command -v sensors &> /dev/null; then
    if [ ! -f /etc/sensors3.conf ] || ! sensors &> /dev/null; then
        echo -e "${BLUE}Configuring sensors...${NC}"
        read -p "Run sensors-detect? (y/n) " -n 1 -r
        echo ""
        
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo sensors-detect --auto
            echo -e "${GREEN}✓ Sensors configured${NC}"
        fi
    fi
fi

echo ""

# Setup ThinkPad fan control
if lsmod | grep -q thinkpad_acpi; then
    if [ ! -f /etc/modprobe.d/thinkpad_acpi.conf ]; then
        echo -e "${BLUE}Setting up ThinkPad fan control...${NC}"
        read -p "Enable fan control? (y/n) " -n 1 -r
        echo ""
        
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo "options thinkpad_acpi fan_control=1" | sudo tee /etc/modprobe.d/thinkpad_acpi.conf
            echo ""
            echo -e "${GREEN}✓ Fan control enabled${NC}"
            echo -e "${YELLOW}⚠ Please reboot or reload the module:${NC}"
            echo -e "  sudo modprobe -r thinkpad_acpi"
            echo -e "  sudo modprobe thinkpad_acpi"
        fi
    else
        echo -e "${GREEN}✓ ThinkPad fan control already configured${NC}"
    fi
fi

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                            ║${NC}"
echo -e "${BLUE}║                    Setup Complete!                         ║${NC}"
echo -e "${BLUE}║                                                            ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}You can now run ThinkUtils:${NC}"
echo -e "  npm run tauri dev"
echo ""
