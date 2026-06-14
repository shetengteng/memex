const i18n = {
  en: {
    badge: '<span class="badge-dot"></span>Local-first AI Memory Hub',
    heroTitle: 'Your AI conversations,<br>unified and searchable.',
    heroSub: "Memex collects sessions from 7 AI editors, indexes them with full-text search, and exposes your entire history through MCP. Everything stays on your machine.",
    btnGetStarted: "Get Started",
    navFeatures: "Features",
    navScreens: "Screens",
    navHow: "How it Works",
    navSkills: "Skills",
    featuresTitle: "Features",
    featuresSub: "Collect, index, summarize, and recall. Across every editor you use.",
    feat1Title: "Universal Collection",
    feat1Desc: "Auto-collects sessions from Claude Code, Cursor, Codex, OpenCode, Aider, and Cline. Event-driven, 2-second debounce.",
    feat2Title: "Full-Text Search",
    feat2Desc: "SQLite FTS5 with BM25 ranking, time decay, and CJK bigram expansion. Instant search across all AI history.",
    feat3Title: "4-Layer Summarization",
    feat3Desc: "Local Ollama LLM generates chunk, session, project, and daily summaries. No data leaves your machine.",
    feat4Title: "MCP Protocol",
    feat4Desc: "4 tools: search_memory, get_session, list_recent, stats. Every new AI session recalls your full history.",
    feat5Title: "Privacy First",
    feat5Desc: "Auto-redaction of API keys, emails, tokens. Cloud LLM is opt-in. Private sessions stay private.",
    feat6Title: "Embedded Daemon",
    feat6Desc: "HTTP API on :9999 (auto-fallback 10000-10009) lives and dies with the desktop app. No background service to babysit.",
    btnWatchPromo: "Watch the walkthrough",
    shotsTitle: "Inside the App",
    shotsSub: "Five-page desktop window, screenshots follow the UI language toggle in the top-right — synthetic data, real layout.",
    shotTodayTitle: "Cross-project KPI at a glance",
    shotTodayLabel: "Greeting · three-window stats · today's activity histogram · weekly auto-summary · pending reflections.",
    shotLibraryTitle: "Four-axis filtering across 250+ sessions",
    shotLibraryLabel: "Tool · project · time · summary-status sidebar + FTS5 BM25 main list with adapter badges.",
    shotInsightsTitle: "Local-LLM daily / weekly / monthly reports",
    shotInsightsLabel: "Three views (report · reflection · trend), generated offline by Ollama, fully bilingual.",
    shotConnectTitle: "7 collectors · 4 IDE integrations",
    shotConnectLabel: "Event-driven 2s ingestion · one-click MCP / SKILL / Hook injection per IDE.",
    shotSettingsTitle: "Multi-Provider with auto-fallback",
    shotSettingsLabel: "LLM · Preferences · Data · System tabs, prompt templates and redaction switches are live.",
    howTitle: "How it Works",
    howSub: "Three stages. Fully automatic. Zero configuration.",
    step1Title: "Collect",
    step1Desc: "File-system watcher detects session changes from 7 AI editors. Event-driven ingestion with 2s debounce, no polling.",
    step2Title: "Process",
    step2Desc: "Chunk, redact sensitive data, extract metadata, generate 4-layer summaries via local Ollama LLM.",
    step3Title: "Recall",
    step3Desc: "MCP protocol exposes 4 tools. Any editor can search your history, retrieve sessions, and see stats. Cross-editor context.",
    skillsTitle: "Ask in plain English. Get your memory back.",
    skillsSub: "Memex installs as 6 MCP tools into your IDE — type a normal sentence, and the LLM picks the right tool and fills the parameters.",
    skillsDemo1Tag: "Cross-IDE recall",
    skillsDemo1You: "What did I discuss with Cursor on the zoom-docs project last week?",
    skillsDemo2Tag: "Resume project",
    skillsDemo2You: "Pick up where I left off on this repo.",
    skillsDemo3Tag: "Decision lookup",
    skillsDemo3You: "Find the retry strategy we decided on three months ago.",
    skillsDemo4Tag: "Weekly report",
    skillsDemo4You: "List every session from June 1 to June 7.",
    skillsToolsLabel: "6 MCP tools, zero parameter memorization:",
    skillsInstallText: "One-click install into Claude Code · Cursor · Codex · OpenCode from",
    techTitle: "Tech Stack",
    ctaTitle: "Start building your AI memory.",
    ctaSub: "Every conversation has value. Stop losing them.",
    footerDocs: "Docs",
    footerLicense: "MIT License",
  },
  zh: {
    badge: '<span class="badge-dot"></span>\u672C\u5730\u4F18\u5148 AI \u8BB0\u5FC6\u4E2D\u67A2',
    heroTitle: '\u4F60\u7684 AI \u5BF9\u8BDD\uFF0C<br>\u7EDF\u4E00\u4E14\u53EF\u641C\u7D22\u3002',
    heroSub: "Memex \u4ECE 7 \u79CD AI \u7F16\u8F91\u5668\u91C7\u96C6\u4F1A\u8BDD\uFF0C\u5168\u6587\u7D22\u5F15\uFF0C\u901A\u8FC7 MCP \u534F\u8BAE\u66B4\u9732\u5168\u91CF\u5386\u53F2\u3002\u6240\u6709\u6570\u636E\u7559\u5728\u672C\u673A\u3002",
    btnGetStarted: "\u5FEB\u901F\u5F00\u59CB",
    navFeatures: "\u7279\u6027",
    navScreens: "\u754C\u9762",
    navHow: "\u5DE5\u4F5C\u6D41\u7A0B",
    navSkills: "\u6280\u80fd",
    featuresTitle: "\u6838\u5FC3\u7279\u6027",
    featuresSub: "\u91C7\u96C6\u3001\u7D22\u5F15\u3001\u6458\u8981\u3001\u53EC\u56DE\u3002\u8DE8\u8D8A\u4F60\u7528\u7684\u6BCF\u4E00\u4E2A\u7F16\u8F91\u5668\u3002",
    feat1Title: "\u5168\u5E73\u53F0\u91C7\u96C6",
    feat1Desc: "\u81EA\u52A8\u91C7\u96C6 Claude Code\u3001Cursor\u3001Codex\u3001OpenCode\u3001Aider\u3001Cline \u4F1A\u8BDD\u3002\u4E8B\u4EF6\u9A71\u52A8\uFF0C2 \u79D2\u5EF6\u8FDF\u5165\u5E93\u3002",
    feat2Title: "\u5168\u6587\u68C0\u7D22",
    feat2Desc: "SQLite FTS5 + BM25 \u6392\u5E8F + \u65F6\u95F4\u8870\u51CF + \u4E2D\u6587 bigram \u5206\u8BCD\u3002\u8DE8\u6240\u6709 AI \u5386\u53F2\u5373\u65F6\u641C\u7D22\u3002",
    feat3Title: "\u56DB\u7EA7\u667A\u80FD\u6458\u8981",
    feat3Desc: "\u672C\u5730 Ollama LLM \u751F\u6210 chunk\u3001session\u3001project\u3001\u65E5/\u5468\u62A5\u6458\u8981\u3002\u6570\u636E\u4E0D\u51FA\u672C\u673A\u3002",
    feat4Title: "MCP \u534F\u8BAE",
    feat4Desc: "4 \u4E2A\u5DE5\u5177\uFF1Asearch_memory / get_session / list_recent / stats\u3002\u6BCF\u4E2A\u65B0 session \u90FD\u80FD\u53EC\u56DE\u5168\u91CF\u5386\u53F2\u3002",
    feat5Title: "\u9690\u79C1\u4F18\u5148",
    feat5Desc: "\u81EA\u52A8\u8131\u654F API key\u3001email\u3001token\u3002\u4E91\u7AEF LLM \u9700\u663E\u5F0F\u5F00\u542F\u3002\u79C1\u6709 session \u59CB\u7EC8\u79C1\u6709\u3002",
    feat6Title: "\u5185\u5D4C Daemon",
    feat6Desc: "HTTP API \u8DD1\u5728 :9999\uFF08\u88AB\u5360\u7528\u81EA\u52A8 fallback 10000-10009\uFF09\uFF0C\u4E0E\u684C\u9762\u5E94\u7528\u540C\u751F\u5171\u6B7B\uFF0C\u65E0\u9700\u5355\u72EC\u8FD0\u7EF4\u3002",
    btnWatchPromo: "\u770b\u5b8c\u6574\u6f14\u793a",
    shotsTitle: "\u4ea7\u54c1\u754c\u9762\u4e00\u89c8",
    shotsSub: "\u4e94\u5927\u9875\u9762\uff0c\u622a\u56fe\u968f\u53f3\u4e0a\u89d2\u8bed\u8a00\u5207\u6362\u540c\u6b65\u5207\u6362 \u2014\u2014 \u6570\u636e\u662f\u865a\u62df\u7684\uff0c\u5e03\u5c40\u662f\u771f\u7684\u3002",
    shotTodayTitle: "\u4e00\u773c\u770b\u5b8c\u8de8\u9879\u76ee KPI",
    shotTodayLabel: "\u95ee\u5019\u8bed \u00b7 \u4eca\u5929 / \u672c\u5468 / \u672c\u6708\u4e09\u6bb5\u7edf\u8ba1 \u00b7 \u4eca\u65e5\u6d3b\u52a8\u67f1\u56fe \u00b7 \u672c\u5468\u81ea\u52a8\u6458\u8981 \u00b7 \u5f85\u4f60\u53cd\u601d\u7684\u4f1a\u8bdd\u3002",
    shotLibraryTitle: "250+ \u4f1a\u8bdd\u7684\u56db\u7ef4\u7b5b\u9009",
    shotLibraryLabel: "\u5de5\u5177 / \u9879\u76ee / \u65f6\u95f4 / \u6458\u8981\u72b6\u6001 \u4fa7\u680f + FTS5 BM25 \u4e3b\u5217\u8868 + \u9002\u914d\u5668\u5fbd\u6807\u3002",
    shotInsightsTitle: "\u672c\u5730 LLM \u751f\u6210\u65e5\u62a5 / \u5468\u62a5 / \u6708\u62a5",
    shotInsightsLabel: "\u62a5\u544a \u00b7 \u53cd\u601d \u00b7 \u8d8b\u52bf \u4e09\u89c6\u56fe \u00b7 \u672c\u5730 Ollama \u79bb\u7ebf\u751f\u6210 \u00b7 \u53cc\u8bed\u65e0\u7f1d\u5207\u6362\u3002",
    shotConnectTitle: "7 \u91c7\u96c6\u6e90 \u00b7 4 IDE \u96c6\u6210",
    shotConnectLabel: "\u4e8b\u4ef6\u9a71\u52a8 2 \u79d2\u5165\u5e93 \u00b7 \u4e00\u952e\u6ce8\u5165 MCP / SKILL / Hook \u5230\u6bcf\u4e2a IDE\u3002",
    shotSettingsTitle: "\u591a Provider \u94fe\u8def \u00b7 \u81ea\u52a8 fallback",
    shotSettingsLabel: "LLM / \u504f\u597d / \u6570\u636e / \u7cfb\u7edf \u56db\u4e2a\u5206\u7ec4 \u00b7 \u63d0\u793a\u8bcd\u6a21\u677f\u4e0e\u8131\u654f\u5f00\u5173\u5b9e\u65f6\u751f\u6548\u3002",
    howTitle: "\u5DE5\u4F5C\u6D41\u7A0B",
    howSub: "\u4E09\u4E2A\u9636\u6BB5\u3002\u5168\u81EA\u52A8\u3002\u96F6\u914D\u7F6E\u3002",
    step1Title: "\u91C7\u96C6",
    step1Desc: "\u6587\u4EF6\u7CFB\u7EDF watcher \u76D1\u542C 7 \u79CD AI \u7F16\u8F91\u5668\u4F1A\u8BDD\u53D8\u5316\u3002\u4E8B\u4EF6\u9A71\u52A8\uFF0C2 \u79D2\u53BB\u6296\uFF0C\u96F6\u8F6E\u8BE2\u3002",
    step2Title: "\u5904\u7406",
    step2Desc: "\u5206\u5757\u3001\u8131\u654F\u3001\u63D0\u53D6\u5143\u6570\u636E\uFF0COllama \u672C\u5730 LLM \u751F\u6210\u56DB\u7EA7\u6458\u8981\u3002",
    step3Title: "\u53EC\u56DE",
    step3Desc: "MCP \u534F\u8BAE\u66B4\u9732 4 \u4E2A\u5DE5\u5177\u3002\u4EFB\u4F55\u7F16\u8F91\u5668\u90FD\u80FD\u641C\u7D22\u5386\u53F2\u3001\u8BFB\u53D6 session\u3001\u67E5\u770B\u7EDF\u8BA1\u3002",
    skillsTitle: "\u7528\u81ea\u7136\u8bed\u8a00\u63d0\u95ee\uff0c\u628a\u8bb0\u5fc6\u62ff\u56de\u6765\u3002",
    skillsSub: "Memex \u5728\u4f60\u7684 IDE \u91cc\u6ce8\u5165 6 \u4e2a MCP \u5de5\u5177\u2014\u2014\u76f4\u63a5\u7528\u65e5\u5e38\u5bf9\u8bdd\u63d0\u95ee\uff0cIDE \u7684 LLM \u81ea\u52a8\u9009\u5de5\u5177\u3001\u81ea\u52a8\u586b\u53c2\u6570\u3002",
    skillsDemo1Tag: "\u8de8 IDE \u68c0\u7d22",
    skillsDemo1You: "\u6211\u4e0a\u5468\u548c Cursor \u5728 zoom-docs \u9879\u76ee\u8ba8\u8bba\u4e86\u4ec0\u4e48\uff1f",
    skillsDemo2Tag: "\u7ee7\u7eed\u4e0a\u6b21\u7684\u4e8b",
    skillsDemo2You: "\u63a5\u7740\u8fd9\u4e2a\u4ed3\u5e93\u4e0a\u6b21\u6ca1\u505a\u5b8c\u7684\u4e8b\u3002",
    skillsDemo3Tag: "\u51b3\u7b56\u56de\u6eaf",
    skillsDemo3You: "\u4e09\u4e2a\u6708\u524d\u6211\u4eec\u5b9a\u7684 retry \u7b56\u7565\u662f\u54ea\u4e00\u79cd\uff1f",
    skillsDemo4Tag: "\u5468\u62a5",
    skillsDemo4You: "\u628a 6 \u6708 1 \u53f7\u5230 6 \u6708 7 \u53f7\u7684\u6240\u6709\u4f1a\u8bdd\u5217\u51fa\u6765\u3002",
    skillsToolsLabel: "6 \u4e2a MCP \u5de5\u5177\uff0c\u96f6\u53c2\u6570\u8bb0\u5fc6\u8d1f\u62c5\uff1a",
    skillsInstallText: "\u5728 Claude Code \u00b7 Cursor \u00b7 Codex \u00b7 OpenCode \u4e00\u952e\u5b89\u88c5\uff0c\u8def\u5f84\uff1a",
    techTitle: "\u6280\u672F\u6808",
    ctaTitle: "\u5F00\u59CB\u6784\u5EFA\u4F60\u7684 AI \u8BB0\u5FC6\u3002",
    ctaSub: "\u6BCF\u4E00\u6B21\u5BF9\u8BDD\u90FD\u6709\u4EF7\u503C\u3002\u522B\u518D\u4E22\u5931\u4E86\u3002",
    footerDocs: "\u6587\u6863",
    footerLicense: "MIT \u534F\u8BAE",
  },
};

const techStack = [
  { name: "Rust", desc: "core, CLI, MCP" },
  { name: "SQLite", desc: "FTS5" },
  { name: "Tauri 2", desc: "desktop" },
  { name: "Vue 3", desc: "tray UI" },
  { name: "Ollama", desc: "local LLM" },
  { name: "MCP", desc: "protocol" },
];

const adapters = [
  { name: "Claude Code", color: "oklch(55% 0.22 270)" },
  { name: "Cursor",      color: "oklch(60% 0.2 155)" },
  { name: "Codex",       color: "oklch(65% 0.18 85)" },
  { name: "OpenCode",    color: "oklch(55% 0.2 290)" },
  { name: "Aider",       color: "oklch(65% 0.15 230)" },
  { name: "Cline",       color: "oklch(65% 0.18 45)" },
];

let currentLang = "en";
let currentTheme = "light";

function applyI18n(lang) {
  const t = i18n[lang];
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    if (t[key] !== undefined) el.innerHTML = t[key];
  });
  document.querySelectorAll("[data-i18n-nav]").forEach((el) => {
    const key = el.getAttribute("data-i18n-nav");
    if (t[key] !== undefined) el.textContent = t[key];
  });
}

function setLang(lang) {
  currentLang = lang;
  applyI18n(lang);
  document.getElementById("btn-lang").textContent = lang === "en" ? "\u4E2D\u6587" : "EN";
  // 让 Screenshots Carousel 的 zh / en 截图跟着 UI 语言切换（CSS toggle，不重写 src）
  const track = document.getElementById("screens-carousel");
  if (track) track.dataset.lang = lang;
}

function toggleLang() { setLang(currentLang === "en" ? "zh" : "en"); }

function toggleTheme() {
  currentTheme = currentTheme === "light" ? "dark" : "light";
  document.documentElement.setAttribute("data-theme", currentTheme === "dark" ? "dark" : "");
  const icon = document.getElementById("icon-theme");
  icon.innerHTML = currentTheme === "dark"
    ? '<circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>'
    : '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>';
}

function renderAdapters() {
  document.getElementById("adapters-list").innerHTML = adapters
    .map(a => `<span class="adapter-pill"><span class="adapter-dot" style="background:${a.color}"></span>${a.name}</span>`)
    .join("");
}

function renderTech() {
  document.getElementById("tech-list").innerHTML = techStack
    .map(t => `<div class="tech-tag">${t.name}<span>${t.desc}</span></div>`)
    .join("");
}

function initReveal() {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    document.querySelectorAll(".reveal").forEach(el => el.classList.add("visible"));
    return;
  }
  const obs = new IntersectionObserver((entries) => {
    entries.forEach(e => {
      if (e.isIntersecting) { e.target.classList.add("visible"); obs.unobserve(e.target); }
    });
  }, { threshold: 0.1, rootMargin: "0px 0px -32px 0px" });
  document.querySelectorAll(".reveal").forEach(el => obs.observe(el));
}

function initNavScroll() {
  document.querySelectorAll('.nav-link[href^="#"]').forEach(link => {
    link.addEventListener("click", e => {
      e.preventDefault();
      const t = document.querySelector(link.getAttribute("href"));
      if (t) window.scrollTo({ top: t.offsetTop - 64, behavior: "smooth" });
    });
  });
}

function initCardGlow() {
  document.querySelectorAll('.feat, .step-card').forEach(card => {
    card.addEventListener('mousemove', e => {
      const rect = card.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      card.style.setProperty('--glow-x', `${x}px`);
      card.style.setProperty('--glow-y', `${y}px`);
    });
  });
}

// Screenshots carousel — snap scroll + dot indicators + prev/next + keyboard ←→
function initCarousel() {
  const track = document.getElementById('screens-carousel');
  const dotsRoot = document.getElementById('screens-dots');
  if (!track || !dotsRoot) return;
  const cards = Array.from(track.querySelectorAll('.carousel-card'));
  if (cards.length === 0) return;

  let idx = 0;
  const setActiveDot = (i) => {
    dotsRoot.querySelectorAll('.carousel-dot').forEach((d, j) => d.classList.toggle('is-active', j === i));
  };

  cards.forEach((_, i) => {
    const d = document.createElement('button');
    d.className = 'carousel-dot' + (i === 0 ? ' is-active' : '');
    d.type = 'button';
    d.setAttribute('aria-label', `slide ${i + 1}`);
    d.addEventListener('click', () => go(i));
    dotsRoot.appendChild(d);
  });

  function go(i) {
    idx = (i + cards.length) % cards.length;
    cards[idx].scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'start' });
    setActiveDot(idx);
  }

  document.querySelector('.carousel-prev')?.addEventListener('click', () => go(idx - 1));
  document.querySelector('.carousel-next')?.addEventListener('click', () => go(idx + 1));

  // Manual scroll → sync dots (with debounce)
  let scrollTimer;
  track.addEventListener('scroll', () => {
    clearTimeout(scrollTimer);
    scrollTimer = setTimeout(() => {
      const w = cards[0].offsetWidth + 24; // include gap
      const i = Math.round(track.scrollLeft / w);
      if (i !== idx && i >= 0 && i < cards.length) {
        idx = i;
        setActiveDot(idx);
      }
    }, 120);
  });

  // Keyboard left/right when section is in view
  window.addEventListener('keydown', (e) => {
    const section = document.getElementById('screens');
    if (!section) return;
    const rect = section.getBoundingClientRect();
    if (rect.bottom < 0 || rect.top > window.innerHeight) return;
    if (e.key === 'ArrowLeft') { e.preventDefault(); go(idx - 1); }
    else if (e.key === 'ArrowRight') { e.preventDefault(); go(idx + 1); }
  });
}

document.addEventListener("DOMContentLoaded", () => {
  setLang("en");
  renderAdapters();
  renderTech();
  initReveal();
  initNavScroll();
  initCardGlow();
  initCarousel();
  if (window.matchMedia("(prefers-color-scheme: dark)").matches) toggleTheme();
});
