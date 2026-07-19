#!/bin/bash
set -e

echo "Setting up passwordless access for ThinkUtils..."
echo ""

# Get the current user
CURRENT_USER=$(whoami)

# Check if user is in sudo or wheel group
if groups $CURRENT_USER | grep -qE '\b(sudo|wheel)\b'; then
    echo "✓ User '$CURRENT_USER' is in sudo/wheel group"
else
    echo "✗ User '$CURRENT_USER' is NOT in sudo/wheel group"
    echo "  You need to be in the sudo or wheel group for passwordless access"
    exit 1
fi

# Install dedicated fan control helper (validates commands before applying)
echo "Installing fan control helper..."
sudo tee /usr/local/bin/thinkutils-fan-control > /dev/null << 'EOF'
#!/bin/bash
set -e
FAN="/proc/acpi/ibm/fan"
case "$1" in
    "level auto"|"level full-speed"|"level 0"|"level 1"|"level 2"|"level 3"|"level 4"|"level 5"|"level 6"|"level 7")
        echo "$1" > "$FAN"
        ;;
    *)
        echo "Invalid command" >&2
        exit 1
        ;;
esac
EOF
sudo chmod 755 /usr/local/bin/thinkutils-fan-control
echo "✓ Helper installed at /usr/local/bin/thinkutils-fan-control"

# Create polkit rules directory if it doesn't exist
sudo mkdir -p /etc/polkit-1/rules.d

# Create the rule file (only allows the dedicated helper, not arbitrary bash)
echo "Creating polkit rule..."
sudo tee /etc/polkit-1/rules.d/50-thinkutils.rules > /dev/null << 'EOF'
/* ThinkUtils: Allow passwordless fan control via dedicated helper only */
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.policykit.exec") {
        var program = action.lookup("program");
        if (program == "/usr/local/bin/thinkutils-fan-control") {
            if (subject.isInGroup("wheel") || subject.isInGroup("sudo")) {
                polkit.log("ThinkUtils: Allowing passwordless fan control for " + subject.user);
                return polkit.Result.YES;
            }
        }
    }
});
EOF

echo "✓ Polkit rule created at /etc/polkit-1/rules.d/50-thinkutils.rules"
echo ""
echo "Reloading polkit..."
sudo systemctl reload polkit 2>/dev/null || sudo killall -HUP polkitd 2>/dev/null || true

echo ""
echo "✓ Setup complete!"
echo ""
echo "ThinkUtils fan control will now work without asking for a password."
echo "Restart the app to test."
