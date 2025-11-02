// About Dialog
export function setupAboutDialog() {
  const aboutLink = document.getElementById('about-link');
  const closeAbout = document.getElementById('close-about');

  if (aboutLink) {
    aboutLink.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      showAbout();
    });
  }

  if (closeAbout) {
    closeAbout.addEventListener('click', closeAbout);
  }
}

function showAbout() {
  console.log('[About] Opening dialog');
  const dialog = document.getElementById('about-dialog');
  if (dialog) {
    dialog.style.display = 'flex';

    if (!dialog.hasAttribute('data-listener')) {
      dialog.setAttribute('data-listener', 'true');
      dialog.addEventListener('click', (e) => {
        if (e.target === dialog) {
          closeAbout();
        }
      });
    }

    const escapeHandler = (e) => {
      if (e.key === 'Escape') {
        closeAbout();
        document.removeEventListener('keydown', escapeHandler);
      }
    };
    document.addEventListener('keydown', escapeHandler);

    setupAboutLinks();
  }
}

function closeAbout() {
  const dialog = document.getElementById('about-dialog');
  if (dialog) {
    dialog.style.display = 'none';
  }
}

function setupAboutLinks() {
  const githubLink = document.getElementById('link-github');
  const docsLink = document.getElementById('link-docs');

  if (githubLink && !githubLink.hasAttribute('data-listener')) {
    githubLink.setAttribute('data-listener', 'true');
    githubLink.addEventListener('click', (e) => e.preventDefault());
  }

  if (docsLink && !docsLink.hasAttribute('data-listener')) {
    docsLink.setAttribute('data-listener', 'true');
    docsLink.addEventListener('click', (e) => e.preventDefault());
  }
}
