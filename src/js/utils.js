// Utility Functions
export function showStatus(message, type = 'info') {
  console.log(`[Status] ${type.toUpperCase()}: ${message}`);

  let statusEl = document.getElementById('global-status-banner');
  if (!statusEl) {
    statusEl = document.createElement('div');
    statusEl.id = 'global-status-banner';
    statusEl.style.cssText = `
      position: fixed;
      top: 50px;
      right: 20px;
      z-index: 9999;
      padding: 12px 20px;
      border-radius: 8px;
      font-size: 14px;
      font-weight: 500;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
      max-width: 400px;
    `;
    document.body.appendChild(statusEl);
  }

  // Without a live region this banner is invisible to screen readers, so every
  // success and failure message went unannounced. Errors are assertive because
  // the action failed and the user needs to know now; the rest are polite.
  statusEl.setAttribute('role', 'status');
  statusEl.setAttribute('aria-live', type === 'error' ? 'assertive' : 'polite');

  statusEl.textContent = message;

  const colors = {
    success: { bg: 'rgba(16, 185, 129, 0.15)', border: 'rgba(16, 185, 129, 0.3)', text: '#10B981' },
    error: { bg: 'rgba(239, 68, 68, 0.15)', border: 'rgba(239, 68, 68, 0.3)', text: '#EF4444' },
    info: { bg: 'rgba(59, 130, 246, 0.15)', border: 'rgba(59, 130, 246, 0.3)', text: '#3B82F6' }
  };

  const color = colors[type] || colors.info;
  statusEl.style.background = color.bg;
  statusEl.style.border = `1px solid ${color.border}`;
  statusEl.style.color = color.text;
  statusEl.style.display = 'block';

  const timeout = type === 'error' ? 10000 : 5000;

  setTimeout(() => {
    if (statusEl.textContent === message) {
      statusEl.style.display = 'none';
    }
  }, timeout);
}

/**
 * Escape text for safe interpolation into innerHTML.
 *
 * Several views render strings that originate outside the app — process names
 * from `ps aux`, mount points, network interface names, ClamAV threat names.
 * Any local user can create a process named `<img src=x onerror=...>`, and with
 * `withGlobalTauri` enabled that script would reach the full `__TAURI__` API.
 *
 * Lives here rather than in one view because it was previously private to
 * security.js, so every other view rendering untrusted strings had no escaping
 * at all.
 */
export function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text ?? '';
  return div.innerHTML;
}
