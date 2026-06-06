/*
 * Memex Prototype Shared JS
 * - 初始化 lucide 图标
 * - 给所有原型注入统一的快捷交互（tab 切换、sheet 开关、cmd-k）
 */

(function () {
  function init() {
    if (window.lucide && typeof window.lucide.createIcons === 'function') {
      window.lucide.createIcons();
    }

    document.querySelectorAll('[data-tabs]').forEach((root) => {
      const triggers = root.querySelectorAll('[data-tab-trigger]');
      const panels = root.querySelectorAll('[data-tab-panel]');
      triggers.forEach((trigger) => {
        trigger.addEventListener('click', () => {
          const key = trigger.getAttribute('data-tab-trigger');
          triggers.forEach((t) => t.setAttribute('aria-selected', String(t === trigger)));
          panels.forEach((p) => {
            p.hidden = p.getAttribute('data-tab-panel') !== key;
          });
        });
      });
    });

    document.querySelectorAll('[data-sheet-open]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const target = document.querySelector(btn.getAttribute('data-sheet-open'));
        if (target) target.hidden = false;
      });
    });
    document.querySelectorAll('[data-sheet-close]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const target = btn.closest('[data-sheet]');
        if (target) target.hidden = true;
      });
    });

    const cmd = document.querySelector('[data-cmd-dialog]');
    const cmdOverlay = document.querySelector('[data-cmd-overlay]');
    function openCmd() { if (cmd) { cmd.hidden = false; cmdOverlay && (cmdOverlay.hidden = false); } }
    function closeCmd() { if (cmd) { cmd.hidden = true; cmdOverlay && (cmdOverlay.hidden = true); } }
    document.querySelectorAll('[data-cmd-trigger]').forEach((el) => el.addEventListener('click', openCmd));
    document.querySelectorAll('[data-cmd-close]').forEach((el) => el.addEventListener('click', closeCmd));
    document.addEventListener('keydown', (e) => {
      const isCmdK = (e.key === 'k' || e.key === 'K') && (e.metaKey || e.ctrlKey);
      if (isCmdK) { e.preventDefault(); cmd && (cmd.hidden ? openCmd() : closeCmd()); }
      if (e.key === 'Escape') closeCmd();
    });

    document.querySelectorAll('[data-collapsible]').forEach((root) => {
      const trigger = root.querySelector('[data-collapsible-trigger]');
      const content = root.querySelector('[data-collapsible-content]');
      if (trigger && content) {
        trigger.addEventListener('click', () => {
          const open = root.getAttribute('data-open') === 'true';
          root.setAttribute('data-open', String(!open));
          content.hidden = open;
        });
      }
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
