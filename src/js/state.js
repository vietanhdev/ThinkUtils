// Application State Management
export const state = {
  currentFanMode: 'auto',
  updateInterval: null,
  fanControlInProgress: false,
  lastFanSpeedSet: null,
  currentView: 'home',
  monitorInterval: null
};

export function setState(key, value) {
  state[key] = value;
}

export function getState(key) {
  return state[key];
}
