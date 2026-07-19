#!/bin/bash
# Removes the legacy ThinkUtils polkit policy.
#
# Older versions installed /usr/share/polkit-1/actions/com.thinkutils.policy,
# which redefined the SHARED action org.freedesktop.policykit.exec with
# allow_active=auth_admin_keep. Had it taken effect it would have opened a
# standing window in which any pkexec invocation -- not just ThinkUtils' --
# was authorised without a prompt.
#
# In practice polkit resolved the action to its own definition, so the
# override did not apply. That outcome depended on file ordering rather than
# on anything guaranteed, so the file is removed rather than relied upon.
#
# Fan control does not need it: setup_permissions() installs a JS rule that
# grants passwordless exec to the fan helper alone.

set -euo pipefail

POLICY="/usr/share/polkit-1/actions/com.thinkutils.policy"

if [ "$(id -u)" -ne 0 ]; then
    echo "Run as root: sudo $0" >&2
    exit 1
fi

if [ ! -e "$POLICY" ]; then
    echo "Nothing to do: $POLICY is not present."
    exit 0
fi

rm -f "$POLICY"
echo "Removed $POLICY"

# Reload so the change applies without a reboot.
systemctl reload polkit 2>/dev/null || killall -HUP polkitd 2>/dev/null || true
echo "Done. Fan control is unaffected -- it uses the helper-scoped polkit rule."
