// Navigation and View Switching
import { elements } from './dom.js';
import { setState, getState } from './state.js';
import { VIEWS, getView, viewElement } from './views/registry.js';

export function setupFeatureNavigation() {
  const menuItems = document.querySelectorAll('.menu-item');

  menuItems.forEach((item) => {
    item.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();

      if (item.classList.contains('disabled')) {
        console.log('[Navigation] Feature disabled:', item.dataset.feature);
        return;
      }

      const feature = item.dataset.feature;
      console.log('[Navigation] Switching to:', feature);

      menuItems.forEach((i) => {
        i.classList.remove('active');
        i.removeAttribute('aria-current');
      });
      item.classList.add('active');
      // Screen readers announce the active item only if it is marked as such;
      // a CSS class alone says nothing to assistive technology.
      item.setAttribute('aria-current', 'page');

      switchView(feature);
    });
  });
}

export function switchView(id) {
  const next = getView(id);
  if (!next) {
    console.warn('[Navigation] Unknown view:', id);
    return;
  }

  const previous = getView(getState('currentView'));

  // Tear down before building up. Without this every view's timers kept running
  // for the life of the app -- three concurrent poll loops while sitting on one
  // page, each rebuilding its DOM on every tick.
  if (previous && previous.id !== next.id && previous.onHide) {
    try {
      previous.onHide();
    } catch (error) {
      // A failing teardown must not block navigation, or the user is stuck.
      console.error(`[Navigation] Failed to tear down ${previous.id}:`, error);
    }
  }

  for (const view of VIEWS) {
    const el = viewElement(view);
    if (el) {
      el.style.display = view.id === next.id ? view.display : 'none';
    }
  }

  setState('currentView', next.id);

  if (elements.pageTitle) {
    elements.pageTitle.textContent = next.title;
  }
  if (elements.pageSubtitle) {
    elements.pageSubtitle.textContent = next.subtitle;
  }

  if (next.onShow) {
    try {
      next.onShow();
    } catch (error) {
      console.error(`[Navigation] Failed to initialise ${next.id}:`, error);
    }
  }
}
