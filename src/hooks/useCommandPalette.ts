import { useState, useEffect } from 'react';

export function useCommandPalette() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  useEffect(() => {
    const openHandler = () => setOpen(true);
    const toggleHandler = () => setOpen((prev) => !prev);
    window.addEventListener('aqbot:open-command-palette', openHandler);
    window.addEventListener('aqbot:toggle-command-palette', toggleHandler);
    return () => {
      window.removeEventListener('aqbot:open-command-palette', openHandler);
      window.removeEventListener('aqbot:toggle-command-palette', toggleHandler);
    };
  }, []);

  return { open, setOpen };
}
