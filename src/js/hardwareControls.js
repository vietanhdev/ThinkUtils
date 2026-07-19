// Hardware control actions, shared by the Home and Performance views.
//
// These were implemented twice, and the copies had drifted:
//
//   - Home disabled its governor buttons during the call and awaited a 500ms
//     settle before refreshing; Performance did neither, so a fast double-click
//     could fire two governor changes and the read-back could land before the
//     kernel had applied the first.
//   - Home rebound its turbo handler on every render without removing the old
//     one; Performance bound once at setup.
//   - The two used different success wording for the same action.
//
// One implementation, one behaviour. Each action takes a callback to refresh
// whichever view invoked it.

import { showStatus } from './utils.js';

const { invoke } = window.__TAURI__.core;

/**
 * The kernel needs a moment to apply a governor change before a read-back
 * reflects it. Without this the UI reads the old value and appears to have
 * ignored the click.
 */
const GOVERNOR_SETTLE_MS = 500;

/**
 * Run a privileged hardware action with consistent status reporting.
 *
 * `busy` elements are disabled for the duration — the governor path could
 * otherwise be triggered twice concurrently, and each call spawns a pkexec.
 */
async function runAction({ pending, success, invokeName, args, refresh, busy = [] }) {
  busy.forEach((el) => {
    if (el) {
      el.disabled = true;
    }
  });

  try {
    showStatus(pending, 'info');
    const response = await invoke(invokeName, args);

    if (response.success) {
      showStatus(success, 'success');
      return true;
    }

    // The backend now returns actionable errors (a missing kernel module reads
    // differently from a denied permission), so surface it rather than a
    // generic failure string.
    showStatus(`Error: ${response.error ?? 'Action failed'}`, 'error');
    return false;
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
    return false;
  } finally {
    busy.forEach((el) => {
      if (el) {
        el.disabled = false;
      }
    });
    if (refresh) {
      await refresh();
    }
  }
}

export function setPowerProfile(profile, refresh) {
  return runAction({
    pending: `Setting power profile to ${profile}...`,
    success: `Power profile set to ${profile}`,
    invokeName: 'set_power_profile',
    args: { profile },
    refresh
  });
}

export async function setCpuGovernor(governor, refresh, busy = []) {
  const ok = await runAction({
    pending: `Setting CPU governor to ${governor}...`,
    success: `CPU governor set to ${governor}`,
    invokeName: 'set_cpu_governor',
    args: { governor },
    busy
  });

  if (ok) {
    await new Promise((resolve) => setTimeout(resolve, GOVERNOR_SETTLE_MS));
  }
  if (refresh) {
    await refresh();
  }
  return ok;
}

export async function setTurboBoost(enabled, refresh, toggleEl) {
  const ok = await runAction({
    pending: `${enabled ? 'Enabling' : 'Disabling'} turbo boost...`,
    success: `Turbo boost ${enabled ? 'enabled' : 'disabled'}`,
    invokeName: 'set_turbo_boost',
    args: { enabled },
    refresh
  });

  // A checkbox that stays flipped after a failed write tells the user the
  // opposite of what happened.
  if (!ok && toggleEl) {
    toggleEl.checked = !enabled;
  }
  return ok;
}

/**
 * Bind a handler once, replacing any previous binding.
 *
 * Home re-ran its setup on every render and bound a fresh listener each time,
 * so a toggle fired N times after N refreshes. Cloning drops every existing
 * listener without needing a reference to the old one.
 */
export function bindOnce(element, event, handler) {
  if (!element) {
    return null;
  }
  const fresh = element.cloneNode(true);
  element.parentNode.replaceChild(fresh, element);
  fresh.addEventListener(event, handler);
  return fresh;
}
