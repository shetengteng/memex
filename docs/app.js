const i18n = {
  en: {
    badge: '<span class="badge-dot"></span>Local-first AI Memory Hub',
    heroTitle: 'Your AI conversations,<br>unified and searchable.',
    heroSub: "Memex collects sessions from 7 AI editors, indexes them with full-text search, and exposes your entire history through MCP. Everything stays on your machine.",
    btnGetStarted: "Get Started",
    navFeatures: "Features",
    navScreens: "Screens",
    navHow: "How it Works",
    featuresTitle: "Features",
    featuresSub: "Collect, index, summarize, and recall. Across every editor you use.",
    feat1Title: "Universal Collection",
    feat1Desc: "Auto-collects sessions from Claude Code, Cursor, Codex, OpenCode, Aider, Continue, and Cline. Event-driven, 2-second debounce.",
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
    shotsTitle: "Inside the App",
    shotsSub: "Tray popup, five-page desktop window, and IDE integration — fake data, real layout.",
    trayToday: "Today",
    trayWeek: "7d",
    trayAll: "All",
    trayRecent: "Recent sessions",
    trayOpen: "Open Memex",
    traySettings: "Settings",
    navToday: "Today",
    navLibrary: "Library",
    navInsights: "Insights",
    navConnect: "Connect",
    navSettings: "Settings",
    connText: "IDEs wired up",
    settingsPrivacy: "Privacy",
    settingsAutoRedact: "Auto-redact API keys & tokens",
    settingsSkipPrivate: "Hide private sessions from MCP",
    settingsNotif: "Notifications",
    shotTrayLabel: "360 × 520 · ⌘⇧M toggles main window",
    shotTodayLabel: "Cross-project ⌘K palette + today's activity",
    shotLibraryLabel: "Filter by project, drill into session detail",
    shotInsightsLabel: "LLM-generated weekly digest + activity bars",
    shotConnectLabel: "One-click MCP + SKILL into 4 IDEs",
    shotSettingsLabel: "Live privacy switches + notification center",
    howTitle: "How it Works",
    howSub: "Three stages. Fully automatic. Zero configuration.",
    step1Title: "Collect",
    step1Desc: "File-system watcher detects session changes from 7 AI editors. Event-driven ingestion with 2s debounce, no polling.",
    step2Title: "Process",
    step2Desc: "Chunk, redact sensitive data, extract metadata, generate 4-layer summaries via local Ollama LLM.",
    step3Title: "Recall",
    step3Desc: "MCP protocol exposes 4 tools. Any editor can search your history, retrieve sessions, and see stats. Cross-editor context.",
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
    featuresTitle: "\u6838\u5FC3\u7279\u6027",
    featuresSub: "\u91C7\u96C6\u3001\u7D22\u5F15\u3001\u6458\u8981\u3001\u53EC\u56DE\u3002\u8DE8\u8D8A\u4F60\u7528\u7684\u6BCF\u4E00\u4E2A\u7F16\u8F91\u5668\u3002",
    feat1Title: "\u5168\u5E73\u53F0\u91C7\u96C6",
    feat1Desc: "\u81EA\u52A8\u91C7\u96C6 Claude Code\u3001Cursor\u3001Codex\u3001OpenCode\u3001Aider\u3001Continue\u3001Cline \u4F1A\u8BDD\u3002\u4E8B\u4EF6\u9A71\u52A8\uFF0C2 \u79D2\u5EF6\u8FDF\u5165\u5E93\u3002",
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
    shotsTitle: "\u4EA7\u54C1\u754C\u9762\u4E00\u89C8",
    shotsSub: "\u6258\u76D8\u5E94\u7528 \u00B7 \u4E94\u5927\u9875 \u00B7 IDE \u96C6\u6210 \u2014\u2014 \u6570\u636E\u662F\u865A\u62DF\u7684\uFF0C\u5E03\u5C40\u662F\u771F\u7684\u3002",
    trayToday: "\u4ECA\u65E5",
    trayWeek: "\u672C\u5468",
    trayAll: "\u603B\u8BA1",
    trayRecent: "\u6700\u8FD1\u4F1A\u8BDD",
    trayOpen: "\u6253\u5F00 Memex",
    traySettings: "\u8BBE\u7F6E",
    navToday: "\u4ECA\u5929",
    navLibrary: "\u8D44\u6599\u5E93",
    navInsights: "\u6D1E\u5BDF",
    navConnect: "\u8FDE\u63A5",
    navSettings: "\u8BBE\u7F6E",
    connText: "IDE \u5DF2\u63A5\u5165",
    settingsPrivacy: "\u9690\u79C1",
    settingsAutoRedact: "\u5165\u5E93\u524D\u81EA\u52A8\u8131\u654F API Key / Token",
    settingsSkipPrivate: "\u79C1\u6709\u4F1A\u8BDD\u4E0D\u66B4\u9732\u7ED9 MCP",
    settingsNotif: "\u901A\u77E5",
    shotTrayLabel: "360 \u00D7 520 \u00B7 \u2318\u21E7M \u5207\u6362\u4E3B\u7A97\u53E3",
    shotTodayLabel: "\u8DE8\u9879\u76EE \u2318K \u547D\u4EE4\u9762\u677F + \u4ECA\u65E5\u6D3B\u8DC3\u4F1A\u8BDD",
    shotLibraryLabel: "\u6309\u9879\u76EE\u8FC7\u6EE4\uFF0C\u62BD\u5C49\u67E5\u770B\u4F1A\u8BDD\u8BE6\u60C5",
    shotInsightsLabel: "LLM \u751F\u6210\u5468\u62A5 + \u6D3B\u52A8\u67F1\u72B6\u56FE",
    shotConnectLabel: "\u4E00\u952E\u5C06 MCP + SKILL \u6CE8\u5165 4 \u4E2A IDE",
    shotSettingsLabel: "\u9690\u79C1\u5F00\u5173\u5373\u65F6\u751F\u6548 + \u901A\u77E5\u4E2D\u5FC3",
    howTitle: "\u5DE5\u4F5C\u6D41\u7A0B",
    howSub: "\u4E09\u4E2A\u9636\u6BB5\u3002\u5168\u81EA\u52A8\u3002\u96F6\u914D\u7F6E\u3002",
    step1Title: "\u91C7\u96C6",
    step1Desc: "\u6587\u4EF6\u7CFB\u7EDF watcher \u76D1\u542C 7 \u79CD AI \u7F16\u8F91\u5668\u4F1A\u8BDD\u53D8\u5316\u3002\u4E8B\u4EF6\u9A71\u52A8\uFF0C2 \u79D2\u53BB\u6296\uFF0C\u96F6\u8F6E\u8BE2\u3002",
    step2Title: "\u5904\u7406",
    step2Desc: "\u5206\u5757\u3001\u8131\u654F\u3001\u63D0\u53D6\u5143\u6570\u636E\uFF0COllama \u672C\u5730 LLM \u751F\u6210\u56DB\u7EA7\u6458\u8981\u3002",
    step3Title: "\u53EC\u56DE",
    step3Desc: "MCP \u534F\u8BAE\u66B4\u9732 4 \u4E2A\u5DE5\u5177\u3002\u4EFB\u4F55\u7F16\u8F91\u5668\u90FD\u80FD\u641C\u7D22\u5386\u53F2\u3001\u8BFB\u53D6 session\u3001\u67E5\u770B\u7EDF\u8BA1\u3002",
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
  { name: "Continue",    color: "oklch(60% 0.2 340)" },
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

document.addEventListener("DOMContentLoaded", () => {
  setLang("en");
  renderAdapters();
  renderTech();
  initReveal();
  initNavScroll();
  initCardGlow();
  if (window.matchMedia("(prefers-color-scheme: dark)").matches) toggleTheme();
});
