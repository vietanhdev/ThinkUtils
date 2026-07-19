// Collapsible log panel with progressive line reveal.
//
// security.js carried two near-identical copies of this — one for scan logs, one
// for install logs — differing only in element ID prefix and reveal delay
// (30ms vs 50ms). Around 230 lines for one behaviour.
//
// The copies also shared a bug worth naming: each scheduled one setTimeout per
// log line, and nothing cancelled them. Starting a second scan cleared the
// output but left the first run's timers pending, so stale lines interleaved
// into the new output. A scan emitting 2000 lines queued 2000 timers spanning a
// minute.

import { escapeHtml } from './utils.js';

/**
 * Classify a log line for styling.
 *
 * The markers come from the backend's own output, so this stays a simple
 * substring check rather than parsing.
 */
function lineClass(log) {
  if (log.includes('✓')) {
    return 'log-line log-success';
  }
  if (log.includes('✗') || log.includes('ERROR:')) {
    return 'log-line log-error';
  }
  if (log.includes('⚠')) {
    return 'log-line log-warning';
  }
  if (log.includes('⊘')) {
    return 'log-line log-info';
  }
  return 'log-line';
}

export class LogPanel {
  /**
   * @param {string} prefix element id prefix, e.g. 'scan-logs' or 'install-logs'
   * @param {string} toggleId id of the collapse button
   * @param {number} revealDelayMs per-line reveal delay
   */
  constructor(prefix, toggleId, revealDelayMs = 30) {
    this.prefix = prefix;
    this.toggleId = toggleId;
    this.revealDelayMs = revealDelayMs;
    this.timers = [];
    this.toggleBound = false;
  }

  el(suffix) {
    return document.getElementById(suffix ? `${this.prefix}-${suffix}` : this.prefix);
  }

  /** Cancel every pending line reveal. Without this, a previous run's timers
   *  keep firing into the new output. */
  cancelPending() {
    this.timers.forEach(clearTimeout);
    this.timers = [];
  }

  start(title) {
    const section = this.el('section');
    if (!section) {
      return;
    }

    this.cancelPending();

    section.style.display = 'block';

    const content = this.el('content');
    if (content) {
      content.style.display = 'block';
    }

    const titleText = this.el('title-text');
    if (titleText) {
      titleText.textContent = `${title} - In Progress`;
    }

    const spinner = this.el('spinner');
    if (spinner) {
      spinner.style.display = 'inline-block';
    }

    const output = this.el('output');
    if (output) {
      output.innerHTML = '';
      const line = document.createElement('div');
      line.className = 'log-line';
      line.textContent = 'Starting...';
      output.appendChild(line);
    }

    this.bindToggle();
  }

  update(logs) {
    const output = this.el('output');
    if (!output) {
      return;
    }

    this.cancelPending();
    output.innerHTML = '';

    logs.forEach((log, index) => {
      const timer = setTimeout(() => {
        const line = document.createElement('div');
        line.className = lineClass(log);
        // textContent, not innerHTML: these lines carry command output and file
        // paths straight from the scanner.
        line.textContent = log;
        output.appendChild(line);
        output.scrollTop = output.scrollHeight;
      }, index * this.revealDelayMs);
      this.timers.push(timer);
    });
  }

  complete(success) {
    const spinner = this.el('spinner');
    if (spinner) {
      spinner.style.display = 'none';
    }

    const titleText = this.el('title-text');
    if (titleText) {
      const base = titleText.textContent.split(' - ')[0];
      titleText.textContent = success ? `${base} - Complete` : `${base} - Failed`;
    }
  }

  bindToggle() {
    if (this.toggleBound) {
      return;
    }
    const button = document.getElementById(this.toggleId);
    const content = this.el('content');
    if (!button || !content) {
      return;
    }

    button.setAttribute('aria-expanded', 'true');
    button.addEventListener('click', () => {
      const isOpen = content.style.display !== 'none';
      content.style.display = isOpen ? 'none' : 'block';
      button.setAttribute('aria-expanded', String(!isOpen));
    });
    this.toggleBound = true;
  }
}

// Kept for callers that render pre-escaped HTML fragments.
export { escapeHtml };
