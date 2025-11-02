// Fan Curve Editor Module
const { invoke } = window.__TAURI__.core;
import { showStatus } from './utils.js';

// Default fan curve points: [temperature, fan_level]
const DEFAULT_CURVE = [
  { temp: 40, level: 0 },
  { temp: 50, level: 1 },
  { temp: 60, level: 3 },
  { temp: 70, level: 5 },
  { temp: 80, level: 7 }
];

let curvePoints = [...DEFAULT_CURVE];
let canvas, ctx;
let isDragging = false;
let draggedPointIndex = -1;
let currentTemp = 0;

const CANVAS_PADDING = 40;
const POINT_RADIUS = 6;
const TEMP_MIN = 30;
const TEMP_MAX = 95;
const LEVEL_MIN = 0;
const LEVEL_MAX = 7;

export async function initFanCurve() {
  canvas = document.getElementById('fan-curve-canvas');
  if (!canvas) {
    return;
  }

  ctx = canvas.getContext('2d');

  // Load saved curve from backend
  await loadCurveFromBackend();

  // Setup event listeners (only once)
  if (!canvas.dataset.initialized) {
    canvas.addEventListener('mousedown', handleMouseDown);
    canvas.addEventListener('mousemove', handleMouseMove);
    canvas.addEventListener('mouseup', handleMouseUp);
    canvas.addEventListener('mouseleave', handleMouseUp);

    document.getElementById('btn-reset-curve')?.addEventListener('click', resetCurve);
    document.getElementById('btn-save-curve')?.addEventListener('click', saveCurve);

    canvas.dataset.initialized = 'true';
  }

  // Always draw the curve when initializing
  drawCurve();
}

export async function startCurveMode() {
  try {
    // Enable fan curve in backend
    await invoke('enable_fan_curve', { enabled: true });

    // Send current curve points to backend
    await invoke('set_fan_curve', { points: curvePoints });

    // Listen for updates from backend
    const { listen } = window.__TAURI__.event;
    if (!window.fanCurveUnlisten) {
      window.fanCurveUnlisten = await listen('fan-curve-update', (event) => {
        const { temperature, fan_level } = event.payload;
        currentTemp = temperature;

        // Update UI
        document.getElementById('curve-current-temp').textContent = `${temperature}°C`;
        document.getElementById('curve-target-speed').textContent = `Level ${fan_level}`;

        drawCurve();
      });
    }

    console.log('[Fan Curve] Started - backend will handle temperature monitoring');
  } catch (error) {
    console.error('[Fan Curve] Failed to start:', error);
  }
}

export async function stopCurveMode() {
  try {
    // Disable fan curve in backend
    await invoke('enable_fan_curve', { enabled: false });

    // Unlisten from events
    if (window.fanCurveUnlisten) {
      window.fanCurveUnlisten();
      window.fanCurveUnlisten = null;
    }

    console.log('[Fan Curve] Stopped');
  } catch (error) {
    console.error('[Fan Curve] Failed to stop:', error);
  }
}

function drawCurve() {
  if (!ctx || !canvas) {
    return;
  }

  const width = canvas.width;
  const height = canvas.height;

  // Clear canvas
  ctx.clearRect(0, 0, width, height);

  // Draw grid
  ctx.strokeStyle = '#2a2a2a';
  ctx.lineWidth = 1;

  // Vertical grid lines (temperature)
  for (let temp = TEMP_MIN; temp <= TEMP_MAX; temp += 10) {
    const x = tempToX(temp);
    ctx.beginPath();
    ctx.moveTo(x, CANVAS_PADDING);
    ctx.lineTo(x, height - CANVAS_PADDING);
    ctx.stroke();
  }

  // Horizontal grid lines (fan level)
  for (let level = LEVEL_MIN; level <= LEVEL_MAX; level++) {
    const y = levelToY(level);
    ctx.beginPath();
    ctx.moveTo(CANVAS_PADDING, y);
    ctx.lineTo(width - CANVAS_PADDING, y);
    ctx.stroke();
  }

  // Draw axes
  ctx.strokeStyle = '#444';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(CANVAS_PADDING, CANVAS_PADDING);
  ctx.lineTo(CANVAS_PADDING, height - CANVAS_PADDING);
  ctx.lineTo(width - CANVAS_PADDING, height - CANVAS_PADDING);
  ctx.stroke();

  // Draw axis labels
  ctx.fillStyle = '#888';
  ctx.font = '11px system-ui';
  ctx.textAlign = 'center';

  // Temperature labels (X-axis)
  for (let temp = TEMP_MIN; temp <= TEMP_MAX; temp += 10) {
    const x = tempToX(temp);
    ctx.fillText(`${temp}°`, x, height - CANVAS_PADDING + 20);
  }

  // Fan level labels (Y-axis)
  ctx.textAlign = 'right';
  for (let level = LEVEL_MIN; level <= LEVEL_MAX; level++) {
    const y = levelToY(level);
    ctx.fillText(`${level}`, CANVAS_PADDING - 10, y + 4);
  }

  // Draw axis titles
  ctx.fillStyle = '#aaa';
  ctx.font = 'bold 12px system-ui';
  ctx.textAlign = 'center';
  ctx.fillText('Temperature (°C)', width / 2, height - 5);

  ctx.save();
  ctx.translate(15, height / 2);
  ctx.rotate(-Math.PI / 2);
  ctx.fillText('Fan Level', 0, 0);
  ctx.restore();

  // Sort points for drawing
  const sorted = [...curvePoints].sort((a, b) => a.temp - b.temp);

  // Draw curve line
  ctx.strokeStyle = '#3b82f6';
  ctx.lineWidth = 2;
  ctx.beginPath();
  sorted.forEach((point, i) => {
    const x = tempToX(point.temp);
    const y = levelToY(point.level);
    if (i === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  });
  ctx.stroke();

  // Draw current temperature indicator
  if (currentTemp > 0) {
    const x = tempToX(currentTemp);
    ctx.strokeStyle = '#ef4444';
    ctx.lineWidth = 2;
    ctx.setLineDash([5, 5]);
    ctx.beginPath();
    ctx.moveTo(x, CANVAS_PADDING);
    ctx.lineTo(x, height - CANVAS_PADDING);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // Draw points
  sorted.forEach((point) => {
    const x = tempToX(point.temp);
    const y = levelToY(point.level);

    // Point shadow
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.beginPath();
    ctx.arc(x + 1, y + 1, POINT_RADIUS, 0, Math.PI * 2);
    ctx.fill();

    // Point
    ctx.fillStyle = '#3b82f6';
    ctx.beginPath();
    ctx.arc(x, y, POINT_RADIUS, 0, Math.PI * 2);
    ctx.fill();

    // Point border
    ctx.strokeStyle = '#fff';
    ctx.lineWidth = 2;
    ctx.stroke();
  });
}

function tempToX(temp) {
  const width = canvas.width;
  const range = TEMP_MAX - TEMP_MIN;
  return CANVAS_PADDING + ((temp - TEMP_MIN) / range) * (width - 2 * CANVAS_PADDING);
}

function levelToY(level) {
  const height = canvas.height;
  const range = LEVEL_MAX - LEVEL_MIN;
  return height - CANVAS_PADDING - ((level - LEVEL_MIN) / range) * (height - 2 * CANVAS_PADDING);
}

function xToTemp(x) {
  const width = canvas.width;
  const range = TEMP_MAX - TEMP_MIN;
  return TEMP_MIN + ((x - CANVAS_PADDING) / (width - 2 * CANVAS_PADDING)) * range;
}

function yToLevel(y) {
  const height = canvas.height;
  const range = LEVEL_MAX - LEVEL_MIN;
  return LEVEL_MAX - ((y - CANVAS_PADDING) / (height - 2 * CANVAS_PADDING)) * range;
}

function handleMouseDown(e) {
  const rect = canvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;

  // Check if clicking on a point
  for (let i = 0; i < curvePoints.length; i++) {
    const px = tempToX(curvePoints[i].temp);
    const py = levelToY(curvePoints[i].level);
    const dist = Math.sqrt((x - px) ** 2 + (y - py) ** 2);

    if (dist <= POINT_RADIUS + 5) {
      isDragging = true;
      draggedPointIndex = i;
      canvas.style.cursor = 'grabbing';
      break;
    }
  }
}

function handleMouseMove(e) {
  const rect = canvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;

  if (isDragging && draggedPointIndex >= 0) {
    // Update point position
    let newTemp = Math.round(xToTemp(x));
    let newLevel = Math.round(yToLevel(y));

    // Clamp values
    newTemp = Math.max(TEMP_MIN, Math.min(TEMP_MAX, newTemp));
    newLevel = Math.max(LEVEL_MIN, Math.min(LEVEL_MAX, newLevel));

    curvePoints[draggedPointIndex] = { temp: newTemp, level: newLevel };
    drawCurve();
  } else {
    // Check if hovering over a point
    let hovering = false;
    for (let i = 0; i < curvePoints.length; i++) {
      const px = tempToX(curvePoints[i].temp);
      const py = levelToY(curvePoints[i].level);
      const dist = Math.sqrt((x - px) ** 2 + (y - py) ** 2);

      if (dist <= POINT_RADIUS + 5) {
        hovering = true;
        break;
      }
    }
    canvas.style.cursor = hovering ? 'grab' : 'default';
  }
}

function handleMouseUp() {
  isDragging = false;
  draggedPointIndex = -1;
  canvas.style.cursor = 'default';
}

async function resetCurve() {
  curvePoints = [...DEFAULT_CURVE];
  drawCurve();

  // Save the reset curve to backend
  try {
    await invoke('set_fan_curve', { points: curvePoints });
    localStorage.setItem('fanCurve', JSON.stringify(curvePoints));
  } catch (error) {
    console.error('[Fan Curve] Failed to save reset curve:', error);
  }

  showStatus('Curve reset to default', 'info');
}

async function saveCurve() {
  try {
    // Save to backend (which persists to disk)
    await invoke('set_fan_curve', { points: curvePoints });

    // Also save to localStorage as backup
    localStorage.setItem('fanCurve', JSON.stringify(curvePoints));

    showStatus('✓ Curve saved', 'success');
  } catch (error) {
    showStatus(`Error saving curve: ${error}`, 'error');
  }
}

async function loadCurveFromBackend() {
  try {
    const config = await invoke('get_fan_curve');
    if (config && config.points && config.points.length > 0) {
      curvePoints = config.points;
      console.log('[Fan Curve] Loaded from backend:', curvePoints);
    }
  } catch (error) {
    console.error('[Fan Curve] Failed to load from backend:', error);
    // Fallback to localStorage
    try {
      const saved = localStorage.getItem('fanCurve');
      if (saved) {
        curvePoints = JSON.parse(saved);
        console.log('[Fan Curve] Loaded from localStorage');
      }
    } catch (e) {
      console.error('[Fan Curve] Failed to load from localStorage:', e);
    }
  }
}

export function getCurvePoints() {
  return curvePoints;
}

export function setCurvePoints(points) {
  if (Array.isArray(points) && points.length > 0) {
    curvePoints = points;
    drawCurve();
  }
}
