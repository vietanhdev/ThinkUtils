// Security View - Antivirus and Security Settings
const { invoke } = window.__TAURI__.core;

let scanInProgress = false;

export async function loadSecurityStatus() {
  console.log('[Security] Loading security status...');

  try {
    const response = await invoke('get_security_status');

    if (response.success && response.data) {
      updateSecurityUI(response.data);
    } else {
      console.error('[Security] Failed to load status:', response.error);
    }
  } catch (error) {
    console.error('[Security] Error loading status:', error);
  }
}

function updateSecurityUI(status) {
  // Update ClamAV status
  const statusBadge = document.getElementById('clamav-status-badge');
  const statusText = document.getElementById('clamav-status-text');
  const installSection = document.getElementById('clamav-install-section');
  const controlsSection = document.getElementById('security-controls-section');

  if (status.clamav_installed) {
    if (statusBadge) {
      statusBadge.className = 'badge badge-success';
      statusBadge.textContent = 'Installed';
    }
    if (statusText) {
      statusText.textContent = status.clamav_running ? 'Running' : 'Stopped';
    }
    if (installSection) {
      installSection.style.display = 'none';
    }
    if (controlsSection) {
      controlsSection.style.display = 'block';
    }

    // Update database info
    updateElement('db-version', status.database_version);
    updateElement('db-last-update', status.last_update);
    updateElement('db-definitions', status.definitions_count);

    // Show warning if database not initialized
    const dbVersion = document.getElementById('db-version');
    if (
      dbVersion &&
      (status.database_version === 'Not initialized' || status.database_version === 'Unknown')
    ) {
      dbVersion.style.color = '#f59e0b';
      dbVersion.title =
        'Database not initialized. Click "Update Definitions" to download virus definitions.';

      // Show a notification hint
      const updateBtn = document.getElementById('btn-update-definitions');
      if (updateBtn && !updateBtn.classList.contains('pulse-hint')) {
        updateBtn.classList.add('pulse-hint');
        setTimeout(() => updateBtn.classList.remove('pulse-hint'), 3000);
      }
    } else if (dbVersion) {
      dbVersion.style.color = '';
      dbVersion.title = '';
    }
  } else {
    if (statusBadge) {
      statusBadge.className = 'badge badge-error';
      statusBadge.textContent = 'Not Installed';
    }
    if (statusText) {
      statusText.textContent = 'ClamAV not found';
    }
    if (installSection) {
      installSection.style.display = 'flex';
    }
    if (controlsSection) {
      controlsSection.style.display = 'none';
    }
  }
}

function updateElement(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value || 'N/A';
  }
}

export function setupSecurityHandlers() {
  // Refresh status button
  const refreshBtn = document.getElementById('btn-refresh-status');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', async () => {
      refreshBtn.disabled = true;
      const originalText = refreshBtn.innerHTML;
      refreshBtn.innerHTML =
        '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="animation: spin 1s linear infinite;"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2" /></svg> Refreshing...';

      try {
        await loadSecurityStatus();
        showNotification('Status refreshed', 'success');
      } catch (error) {
        showNotification('Failed to refresh status', 'error');
      } finally {
        refreshBtn.disabled = false;
        refreshBtn.innerHTML = originalText;
      }
    });
  }

  // Install ClamAV
  const installBtn = document.getElementById('btn-install-clamav');
  if (installBtn) {
    installBtn.addEventListener('click', async () => {
      installBtn.disabled = true;
      installBtn.textContent = 'Installing...';

      // Show inline installation logs
      showInstallLogs();

      try {
        const response = await invoke('install_clamav');

        // Update logs inline
        if (response.logs && response.logs.length > 0) {
          updateInstallLogs(response.logs);
        }

        if (response.success) {
          showNotification('ClamAV installed successfully', 'success');
          completeInstallLogs(true);
          setTimeout(async () => {
            await loadSecurityStatus();
          }, 2000);
        } else {
          // Check if it's a manual install error
          if (response.error && response.error.startsWith('MANUAL_INSTALL:')) {
            hideInstallLogs();
            const instructions = response.error.replace('MANUAL_INSTALL:', '');
            showManualInstallDialog(instructions);
          } else {
            completeInstallLogs(false);
            showNotification('Installation failed: ' + response.error, 'error');
          }
        }
      } catch (error) {
        completeInstallLogs(false);
        showNotification('Error: ' + error, 'error');
      } finally {
        installBtn.disabled = false;
        installBtn.textContent = 'Install ClamAV';
      }
    });
  }

  // Update definitions
  const updateBtn = document.getElementById('btn-update-definitions');
  if (updateBtn) {
    updateBtn.addEventListener('click', async () => {
      updateBtn.disabled = true;
      const originalText = updateBtn.textContent;
      updateBtn.textContent = 'Updating...';

      try {
        const response = await invoke('update_virus_definitions');

        if (response.success) {
          showNotification('Virus definitions updated', 'success');
          await loadSecurityStatus();
        } else {
          showNotification('Update failed: ' + response.error, 'error');
        }
      } catch (error) {
        showNotification('Error: ' + error, 'error');
      } finally {
        updateBtn.disabled = false;
        updateBtn.textContent = originalText;
      }
    });
  }

  // Quick scan
  const quickScanBtn = document.getElementById('btn-quick-scan');
  if (quickScanBtn) {
    quickScanBtn.addEventListener('click', async () => {
      if (scanInProgress) {
        return;
      }

      scanInProgress = true;
      quickScanBtn.disabled = true;
      quickScanBtn.textContent = 'Scanning...';

      // Show inline scan logs
      showScanLogs('Quick Scan');

      try {
        const response = await invoke('quick_scan');

        if (response.success && response.data) {
          // Update logs inline
          if (response.data.logs && response.data.logs.length > 0) {
            updateScanLogs(response.data.logs);
          }

          displayScanResults(response.data);
          completeScanLogs(true);
          showNotification('Quick scan completed', 'success');
        } else {
          completeScanLogs(false);
          showNotification('Scan failed: ' + response.error, 'error');
        }
      } catch (error) {
        completeScanLogs(false);
        showNotification('Error: ' + error, 'error');
      } finally {
        scanInProgress = false;
        quickScanBtn.disabled = false;
        quickScanBtn.textContent = 'Quick Scan';
      }
    });
  }

  // Custom scan
  const customScanBtn = document.getElementById('btn-custom-scan');
  if (customScanBtn) {
    customScanBtn.addEventListener('click', async () => {
      const pathInput = document.getElementById('scan-path-input');
      const path = pathInput?.value.trim();

      if (!path) {
        showNotification('Please enter a path to scan', 'error');
        return;
      }

      if (scanInProgress) {
        return;
      }

      scanInProgress = true;
      customScanBtn.disabled = true;
      customScanBtn.textContent = 'Scanning...';

      // Show inline scan logs
      showScanLogs('Custom Scan');

      try {
        const response = await invoke('scan_path', { path });

        if (response.success && response.data) {
          // Update logs inline
          if (response.data.logs && response.data.logs.length > 0) {
            updateScanLogs(response.data.logs);
          }

          displayScanResults(response.data);
          completeScanLogs(true);
          showNotification('Scan completed', 'success');
        } else {
          completeScanLogs(false);
          showNotification('Scan failed: ' + response.error, 'error');
        }
      } catch (error) {
        completeScanLogs(false);
        showNotification('Error: ' + error, 'error');
      } finally {
        scanInProgress = false;
        customScanBtn.disabled = false;
        customScanBtn.textContent = 'Scan Path';
      }
    });
  }

  // Browse button for path selection
  const browseBtn = document.getElementById('btn-browse-path');
  if (browseBtn) {
    browseBtn.addEventListener('click', () => {
      // Set home directory as default
      const homeDir = '~';
      const pathInput = document.getElementById('scan-path-input');
      if (pathInput) {
        pathInput.value = homeDir;
      }
    });
  }
}

function displayScanResults(results) {
  const resultsSection = document.getElementById('scan-results');
  const resultsContent = document.getElementById('scan-results-content');

  if (!resultsSection || !resultsContent) {
    return;
  }

  resultsSection.style.display = 'block';

  // Update summary
  updateElement('scan-files-count', results.scanned_files.toString());
  updateElement('scan-threats-count', results.infected_files.toString());
  updateElement('scan-time', results.scan_time);

  // Update threat status
  const threatStatus = document.getElementById('scan-threat-status');
  if (threatStatus) {
    if (results.infected_files > 0) {
      threatStatus.className = 'badge badge-error';
      threatStatus.textContent = 'Threats Found';
    } else {
      threatStatus.className = 'badge badge-success';
      threatStatus.textContent = 'Clean';
    }
  }

  // Display threats list
  const threatsList = document.getElementById('threats-list');
  if (threatsList) {
    if (results.threats && results.threats.length > 0) {
      threatsList.innerHTML = results.threats
        .map(
          (threat) => `
        <div class="threat-item">
          <div class="threat-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
              <line x1="12" y1="9" x2="12" y2="13"/>
              <line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
          </div>
          <div class="threat-info">
            <div class="threat-name">${escapeHtml(threat.threat_name)}</div>
            <div class="threat-path">${escapeHtml(threat.file_path)}</div>
          </div>
        </div>
      `
        )
        .join('');
    } else {
      threatsList.innerHTML = '<div class="no-threats">No threats detected</div>';
    }
  }
}

function showNotification(message, type = 'info') {
  console.log(`[Security] ${type.toUpperCase()}: ${message}`);

  // Create notification element
  const notification = document.createElement('div');
  notification.className = `security-notification ${type}`;
  notification.textContent = message;

  // Add to page
  const container = document.querySelector('.main-content');
  if (container) {
    container.appendChild(notification);

    // Auto remove after 3 seconds
    setTimeout(() => {
      notification.remove();
    }, 3000);
  }
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function showScanLogs(scanType) {
  const logsSection = document.getElementById('scan-logs-section');
  const logsContent = document.getElementById('scan-logs-content');
  const logsOutput = document.getElementById('scan-logs-output');
  const titleText = document.getElementById('scan-logs-title-text');
  const spinner = document.getElementById('scan-logs-spinner');

  if (!logsSection) {
    return;
  }

  // Show the section
  logsSection.style.display = 'block';

  // Expand the content
  if (logsContent) {
    logsContent.style.display = 'block';
  }

  // Update title
  if (titleText) {
    titleText.textContent = `${scanType} - In Progress`;
  }

  // Show spinner
  if (spinner) {
    spinner.style.display = 'inline-block';
  }

  // Clear previous logs
  if (logsOutput) {
    logsOutput.innerHTML = '<div class="log-line">Initializing scan...</div>';
  }

  // Setup toggle button
  setupScanLogsToggle();
}

function updateScanLogs(logs) {
  const output = document.getElementById('scan-logs-output');
  if (!output) {
    return;
  }

  // Clear and add all logs
  output.innerHTML = '';

  logs.forEach((log, index) => {
    setTimeout(() => {
      let className = 'log-line';
      if (log.includes('✓')) {
        className += ' log-success';
      } else if (log.includes('✗')) {
        className += ' log-error';
      } else if (log.includes('⚠')) {
        className += ' log-warning';
      } else if (log.includes('⊘')) {
        className += ' log-info';
      } else if (log.includes('ERROR:')) {
        className += ' log-error';
      }

      const logElement = document.createElement('div');
      logElement.className = className;
      logElement.textContent = log;
      output.appendChild(logElement);

      // Auto-scroll to bottom
      output.scrollTop = output.scrollHeight;
    }, index * 30);
  });
}

function completeScanLogs(success) {
  const titleText = document.getElementById('scan-logs-title-text');
  const spinner = document.getElementById('scan-logs-spinner');

  // Hide spinner
  if (spinner) {
    spinner.style.display = 'none';
  }

  // Update title
  if (titleText) {
    const scanType = titleText.textContent.split(' - ')[0];
    titleText.textContent = success ? `${scanType} - Complete` : `${scanType} - Failed`;
  }
}

function setupScanLogsToggle() {
  const toggleBtn = document.getElementById('btn-toggle-scan-logs');
  const logsContent = document.getElementById('scan-logs-content');

  if (!toggleBtn || !logsContent) {
    return;
  }

  // Remove old listeners
  const newToggleBtn = toggleBtn.cloneNode(true);
  toggleBtn.parentNode.replaceChild(newToggleBtn, toggleBtn);

  newToggleBtn.addEventListener('click', () => {
    const isExpanded = logsContent.style.display !== 'none';

    if (isExpanded) {
      logsContent.style.display = 'none';
      newToggleBtn.classList.add('collapsed');
    } else {
      logsContent.style.display = 'block';
      newToggleBtn.classList.remove('collapsed');
    }
  });
}

function showInstallLogs() {
  const logsSection = document.getElementById('install-logs-section');
  const logsContent = document.getElementById('install-logs-content');
  const logsOutput = document.getElementById('install-logs-output');
  const titleText = document.getElementById('install-logs-title-text');
  const spinner = document.getElementById('install-logs-spinner');

  if (!logsSection) {
    return;
  }

  // Show the section
  logsSection.style.display = 'block';

  // Expand the content
  if (logsContent) {
    logsContent.style.display = 'block';
  }

  // Update title
  if (titleText) {
    titleText.textContent = 'Installation - In Progress';
  }

  // Show spinner
  if (spinner) {
    spinner.style.display = 'inline-block';
  }

  // Clear previous logs
  if (logsOutput) {
    logsOutput.innerHTML = '<div class="log-line">Starting installation...</div>';
  }

  // Setup toggle button
  setupInstallLogsToggle();
}

function updateInstallLogs(logs) {
  const output = document.getElementById('install-logs-output');
  if (!output) {
    return;
  }

  // Clear and add all logs
  output.innerHTML = '';

  logs.forEach((log, index) => {
    setTimeout(() => {
      let className = 'log-line';
      if (log.includes('✓')) {
        className += ' log-success';
      } else if (log.includes('✗')) {
        className += ' log-error';
      } else if (log.includes('⚠')) {
        className += ' log-warning';
      } else if (log.includes('ERROR:')) {
        className += ' log-error';
      }

      const logElement = document.createElement('div');
      logElement.className = className;
      logElement.textContent = log;
      output.appendChild(logElement);

      // Auto-scroll to bottom
      output.scrollTop = output.scrollHeight;
    }, index * 50);
  });
}

function completeInstallLogs(success) {
  const titleText = document.getElementById('install-logs-title-text');
  const spinner = document.getElementById('install-logs-spinner');

  // Hide spinner
  if (spinner) {
    spinner.style.display = 'none';
  }

  // Update title
  if (titleText) {
    titleText.textContent = success ? 'Installation - Complete' : 'Installation - Failed';
  }
}

function hideInstallLogs() {
  const logsSection = document.getElementById('install-logs-section');
  if (logsSection) {
    logsSection.style.display = 'none';
  }
}

function setupInstallLogsToggle() {
  const toggleBtn = document.getElementById('btn-toggle-install-logs');
  const logsContent = document.getElementById('install-logs-content');

  if (!toggleBtn || !logsContent) {
    return;
  }

  // Remove old listeners
  const newToggleBtn = toggleBtn.cloneNode(true);
  toggleBtn.parentNode.replaceChild(newToggleBtn, toggleBtn);

  newToggleBtn.addEventListener('click', () => {
    const isExpanded = logsContent.style.display !== 'none';

    if (isExpanded) {
      logsContent.style.display = 'none';
      newToggleBtn.classList.add('collapsed');
    } else {
      logsContent.style.display = 'block';
      newToggleBtn.classList.remove('collapsed');
    }
  });
}

function showManualInstallDialog(instructions) {
  // Remove any existing dialog
  const existingDialog = document.querySelector('.manual-install-dialog');
  if (existingDialog) {
    existingDialog.remove();
  }

  // Create dialog
  const dialog = document.createElement('div');
  dialog.className = 'manual-install-dialog';
  dialog.innerHTML = `
    <div class="manual-install-overlay"></div>
    <div class="manual-install-content">
      <div class="manual-install-header">
        <h3>Manual Installation Required</h3>
        <button class="manual-install-close">&times;</button>
      </div>
      <div class="manual-install-body">
        <p>Automatic installation is not supported for your Linux distribution. Please install ClamAV manually using the following command:</p>
        <div class="manual-install-command">
          <code>${escapeHtml(instructions)}</code>
          <button class="manual-install-copy" title="Copy to clipboard">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          </button>
        </div>
        <p class="manual-install-note">After installation, click the refresh button below to update the status.</p>
      </div>
      <div class="manual-install-footer">
        <button class="security-button secondary manual-install-cancel">Close</button>
        <button class="security-button primary manual-install-refresh">Refresh Status</button>
      </div>
    </div>
  `;

  document.body.appendChild(dialog);

  // Handle close
  const closeBtn = dialog.querySelector('.manual-install-close');
  const cancelBtn = dialog.querySelector('.manual-install-cancel');
  const overlay = dialog.querySelector('.manual-install-overlay');

  const closeDialog = () => dialog.remove();

  closeBtn.addEventListener('click', closeDialog);
  cancelBtn.addEventListener('click', closeDialog);
  overlay.addEventListener('click', closeDialog);

  // Handle copy
  const copyBtn = dialog.querySelector('.manual-install-copy');
  copyBtn.addEventListener('click', () => {
    navigator.clipboard
      .writeText(instructions)
      .then(() => {
        showNotification('Command copied to clipboard', 'success');
      })
      .catch(() => {
        showNotification('Failed to copy command', 'error');
      });
  });

  // Handle refresh
  const refreshBtn = dialog.querySelector('.manual-install-refresh');
  refreshBtn.addEventListener('click', async () => {
    closeDialog();
    await loadSecurityStatus();
  });
}
