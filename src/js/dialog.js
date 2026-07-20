// Shared modal dialog behaviour.
//
// Both dialogs were plain divs toggled with style.display: no role, no focus
// management, and no consistent way to close them. The About dialog registered a
// fresh Escape listener on `document` every time it opened but only removed it
// inside the Escape branch, so closing via the X button or the overlay left the
// listener attached — open it five times and five handlers fired on the next
// Escape. The permission dialog had no Escape handler at all, which left it
// keyboard-inescapable.

const openDialogs = new Map();

/**
 * Show a dialog as a modal, and return a function that closes it.
 *
 * Focus moves into the dialog and is restored to whatever had it when the dialog
 * closes — without that, dismissing a dialog drops keyboard users back at the
 * top of the document.
 */
export function openDialog(dialogId, { onClose } = {}) {
  const dialog = document.getElementById(dialogId);
  if (!dialog) {
    console.warn('[Dialog] No such dialog:', dialogId);
    return () => {};
  }

  // Re-opening an already-open dialog must not stack a second set of handlers.
  if (openDialogs.has(dialogId)) {
    return openDialogs.get(dialogId);
  }

  const previouslyFocused = document.activeElement;

  dialog.style.display = 'flex';
  dialog.setAttribute('role', 'dialog');
  dialog.setAttribute('aria-modal', 'true');
  dialog.removeAttribute('aria-hidden');

  const focusable = () =>
    Array.from(
      dialog.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      )
    ).filter((el) => !el.disabled && el.offsetParent !== null);

  const onKeyDown = (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }

    // Trap Tab inside the dialog. Without this, tabbing walks out into the page
    // behind the overlay, where the user cannot see what is focused.
    if (e.key !== 'Tab') {
      return;
    }
    const items = focusable();
    if (items.length === 0) {
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];

    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  const onOverlayClick = (e) => {
    if (e.target === dialog) {
      close();
    }
  };

  function close() {
    if (!openDialogs.has(dialogId)) {
      return;
    }
    openDialogs.delete(dialogId);

    document.removeEventListener('keydown', onKeyDown, true);
    dialog.removeEventListener('click', onOverlayClick);

    dialog.style.display = 'none';
    dialog.setAttribute('aria-hidden', 'true');
    dialog.removeAttribute('aria-modal');

    if (previouslyFocused && typeof previouslyFocused.focus === 'function') {
      previouslyFocused.focus();
    }
    if (onClose) {
      onClose();
    }
  }

  // Capture phase so the dialog sees Escape before anything in the page can
  // swallow it.
  document.addEventListener('keydown', onKeyDown, true);
  dialog.addEventListener('click', onOverlayClick);

  const initial = focusable()[0];
  if (initial) {
    initial.focus();
  }

  openDialogs.set(dialogId, close);
  return close;
}

export function closeDialog(dialogId) {
  const close = openDialogs.get(dialogId);
  if (close) {
    close();
  }
}

export function isDialogOpen(dialogId) {
  return openDialogs.has(dialogId);
}
