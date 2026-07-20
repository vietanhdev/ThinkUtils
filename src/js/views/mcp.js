// AI Integration / MCP View
const { invoke } = window.__TAURI__.core;

let mcpSetupDone = false;

/**
 * Whether the server is running, as last reported by the backend.
 *
 * `null` means unknown — the status call failed, so nothing has confirmed the
 * state. Deriving this from the toggle button's label instead meant a single
 * failed status read could strand the UI showing "Starting..." forever while
 * the server ran fine.
 */
let serverRunning = null;

export function setupMcpView() {
  if (mcpSetupDone) {
    return;
  }
  mcpSetupDone = true;

  // Tab switching
  document.querySelectorAll('.mcp-tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      document.querySelectorAll('.mcp-tab').forEach((t) => t.classList.remove('active'));
      document.querySelectorAll('.mcp-tab-content').forEach((c) => c.classList.remove('active'));
      tab.classList.add('active');
      const target = document.getElementById('tab-' + tab.dataset.tab);
      if (target) {
        target.classList.add('active');
      }
    });
  });

  // Copy buttons
  document.querySelectorAll('.btn-copy').forEach((btn) => {
    btn.addEventListener('click', () => {
      const target = document.getElementById(btn.dataset.target);
      if (target) {
        const code = target.querySelector('.mcp-config');
        if (code) {
          navigator.clipboard.writeText(code.textContent.trim());
          btn.textContent = 'Copied!';
          setTimeout(() => {
            btn.textContent = 'Copy';
          }, 2000);
        }
      }
    });
  });

  // Toggle button
  const toggleBtn = document.getElementById('btn-mcp-toggle');
  if (toggleBtn) {
    toggleBtn.addEventListener('click', toggleMcpServer);
  }
}

export async function loadMcpStatus() {
  const dot = document.getElementById('mcp-status-dot');
  const text = document.getElementById('mcp-status-text');
  const btn = document.getElementById('btn-mcp-toggle');
  const hostInput = document.getElementById('mcp-host');
  const portInput = document.getElementById('mcp-port');

  if (!dot || !text || !btn) {
    return;
  }

  try {
    const response = await invoke('get_mcp_status');
    if (response.success && response.data) {
      const { running, host, port, path } = response.data;
      serverRunning = running;
      if (running) {
        dot.className = 'status-dot installed';
        text.textContent = `Running on ${host}:${port}`;
        btn.textContent = 'Stop Server';
        btn.className = 'btn-secondary';
      } else {
        dot.className = 'status-dot not-installed';
        text.textContent = 'Stopped';
        btn.textContent = 'Start Server';
        btn.className = 'btn-primary';
      }
      if (hostInput) {
        hostInput.value = host;
      }
      if (portInput) {
        portInput.value = port;
      }

      // Update config snippets with current host/port
      updateConfigSnippets(host, port, path);
    } else {
      // Backend answered but could not report state. Say so rather than leaving
      // whatever the last render showed.
      serverRunning = null;
      dot.className = 'status-dot not-installed';
      text.textContent = `Status unavailable: ${response.error ?? 'unknown error'}`;
    }
  } catch (error) {
    console.error('[MCP] Status check failed:', error);
    serverRunning = null;
    text.textContent = 'Status unavailable';
  }
}

// The path comes from the backend (McpStatus.path) so these snippets cannot
// drift from the route the router actually serves. They were hardcoded to
// `/sse` from the rmcp 0.1.5 days; rmcp 2 serves Streamable HTTP at `/mcp` and
// nothing at `/sse`, so every pasted config 404'd.
function updateConfigSnippets(host, port, path = '/mcp') {
  const url = `http://${host}:${port}${path}`;

  // Streamable HTTP is "type": "http" -- "sse" selects the transport rmcp 2
  // removed, which fails even against the correct URL.
  const claudeCodeStr = JSON.stringify(
    { mcpServers: { thinkutils: { type: 'http', url } } },
    null,
    2
  );
  // All others use just "url"
  const standardStr = JSON.stringify({ mcpServers: { thinkutils: { url } } }, null, 2);

  const claudeCodeConfig = document.getElementById('config-claude-code');
  if (claudeCodeConfig) {
    claudeCodeConfig.textContent = claudeCodeStr;
  }

  const claudeDesktopConfig = document.getElementById('config-claude-desktop');
  if (claudeDesktopConfig) {
    claudeDesktopConfig.textContent = standardStr;
  }

  const cursorConfig = document.getElementById('config-cursor');
  if (cursorConfig) {
    cursorConfig.textContent = standardStr;
  }

  const lmStudioConfig = document.getElementById('config-lm-studio');
  if (lmStudioConfig) {
    lmStudioConfig.textContent = standardStr;
  }

  const urlDisplay = document.getElementById('mcp-url-display');
  if (urlDisplay) {
    urlDisplay.textContent = url;
  }
}

async function toggleMcpServer() {
  const btn = document.getElementById('btn-mcp-toggle');
  const text = document.getElementById('mcp-status-text');
  if (!btn) {
    return;
  }

  // Read the tracked state, not the button's own label. loadMcpStatus only
  // relabels the button when the status call succeeds, and its catch merely
  // logs -- so one failed status read left the label reading "Starting..."
  // while the server was in fact running. The next click then evaluated
  // "not Stop Server" as "not running" and issued a second start against the
  // bound port, reporting "Address already in use" for a healthy server, with
  // Stop unreachable short of restarting the app.
  if (serverRunning === null) {
    // State unknown after a failed read: resolve it before acting rather than
    // guessing, since guessing wrong is what caused the deadlock.
    await loadMcpStatus();
    if (serverRunning === null) {
      if (text) {
        text.textContent = 'Cannot reach the server status - try again';
      }
      return;
    }
  }

  const isRunning = serverRunning;
  btn.disabled = true;

  try {
    if (isRunning) {
      btn.textContent = 'Stopping...';
      const response = await invoke('stop_mcp_server');
      if (!response.success && text) {
        text.textContent = 'Error: ' + response.error;
      }
    } else {
      btn.textContent = 'Starting...';
      const hostInput = document.getElementById('mcp-host');
      const portInput = document.getElementById('mcp-port');
      const host = hostInput ? hostInput.value : '127.0.0.1';
      const port = portInput ? parseInt(portInput.value) || 8779 : 8779;

      const response = await invoke('start_mcp_server', { host, port });
      if (!response.success && text) {
        text.textContent = 'Error: ' + response.error;
      }
    }
    await loadMcpStatus();
  } catch (error) {
    console.error('[MCP] Toggle failed:', error);
    if (text) {
      text.textContent = 'Error: ' + error;
    }
  } finally {
    btn.disabled = false;
  }
}
