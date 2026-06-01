const API = '';
let allSessionsCache = [];

const ADAPTER_COLORS = {
  claude_code: 'var(--primary)', cursor: 'var(--purple)', codex: 'var(--cyan)',
  opencode: 'var(--green)', aider: 'var(--orange)', continue_dev: '#4ade80', cline: 'var(--pink)',
};

function showPage(name) {
  document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
  document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
  const page = document.getElementById('page-' + name);
  if (page) page.classList.add('active');
  const nav = document.querySelector('[data-page="' + name + '"]');
  if (nav) nav.classList.add('active');
}

function badgeFor(source) {
  return `<span class="badge badge-${source || 'unknown'}">${(source || 'unknown').replace(/_/g, ' ')}</span>`;
}

function relativeTime(dateStr) {
  if (!dateStr) return '-';
  const diff = (Date.now() - new Date(dateStr).getTime()) / 1000;
  if (diff < 60) return 'just now';
  if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
  if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
  if (diff < 604800) return Math.floor(diff / 86400) + 'd ago';
  return new Date(dateStr).toLocaleDateString();
}

function shortId(id) { return id ? (id.length > 12 ? id.slice(0, 12) + '\u2026' : id) : '-'; }

function fmtNum(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return (n || 0).toLocaleString();
}

function escapeHtml(str) {
  const d = document.createElement('div');
  d.textContent = str;
  return d.innerHTML;
}

async function api(path) {
  const resp = await fetch(API + path);
  if (!resp.ok) throw new Error('API error: ' + resp.status);
  return resp.json();
}

function cardHtml(label, value, sub, cls = '') {
  return `<div class="card ${cls}">
    <div class="card-label">${label}</div>
    <div class="card-value">${fmtNum(value)}</div>
    ${sub ? `<div class="card-sub">${sub}</div>` : ''}
  </div>`;
}

// ===== Dashboard =====
async function loadDashboard() {
  try {
    const [stats, breakdown, timeline, ss] = await Promise.all([
      api('/stats'), api('/stats/breakdown'), api('/timeline?days=30'),
      api('/summaries/stats').catch(() => null),
    ]);
    renderStatsCards(stats, ss);
    renderTimeline(timeline.timeline || []);
    renderBreakdown(breakdown);
  } catch (e) {
    document.getElementById('stats-cards').innerHTML =
      `<div class="card"><div class="card-label">Error</div><div class="card-sub">${escapeHtml(e.message)}</div></div>`;
  }
}

function renderStatsCards(stats, ss) {
  const el = document.getElementById('stats-cards');
  let html = cardHtml('Sessions', stats.sessions, fmtNum(stats.sources) + ' sources') +
    cardHtml('Messages', stats.messages, '') +
    cardHtml('Chunks', stats.chunks, 'FTS5 indexed');

  if (ss) {
    const pct = ss.sessions_total > 0 ? Math.round(ss.session_summaries / ss.sessions_total * 100) : 0;
    const providerLabel = ss.llm_provider
      ? `<span class="status-dot status-online"></span>${ss.llm_provider} (${ss.ollama_model || 'unknown'})`
      : '<span class="status-dot status-offline"></span>disabled';
    html += `<div class="card card-accent">
      <div class="card-label">LLM Summaries</div>
      <div class="card-value">${ss.session_summaries}<span style="font-size:14px;color:var(--muted-foreground)">/${ss.sessions_total}</span></div>
      <div class="card-sub">${pct}% sessions &middot; ${fmtNum(ss.chunk_summaries)} chunks</div>
      <div class="card-sub" style="margin-top:8px">${providerLabel}</div>
    </div>`;
  }
  el.innerHTML = html;
}

function renderTimeline(timeline) {
  const el = document.getElementById('timeline-chart');
  if (!timeline.length) { el.innerHTML = '<div class="empty" style="padding:20px">No data</div>'; return; }
  const max = Math.max(...timeline.map(d => d.sessions || 0), 1);
  el.innerHTML = timeline.map(d => {
    const v = d.sessions || 0;
    const pct = Math.max((v / max) * 100, 2);
    const adapters = d.by_adapter ? Object.entries(d.by_adapter).map(([k, c]) => k + ': ' + c).join(', ') : '';
    return `<div class="bar-col">
      <div class="bar" style="height:${pct}%">
        <div class="bar-tooltip">${d.date}<br>${v} sessions${adapters ? '<br>' + adapters : ''}</div>
      </div>
      <div class="bar-label">${(d.date || '').slice(5)}</div>
    </div>`;
  }).join('');
}

function renderBreakdown(bd) {
  const el = document.getElementById('breakdown-row');
  let html = '';

  if (bd.by_adapter) {
    const total = Object.values(bd.by_adapter).reduce((a, b) => a + b, 0) || 1;
    html += '<div class="pie-card"><h3>By Source</h3><div class="pie-items">';
    for (const [name, count] of Object.entries(bd.by_adapter).sort((a, b) => b[1] - a[1])) {
      const pct = (count / total * 100).toFixed(0);
      const color = ADAPTER_COLORS[name] || 'var(--muted-foreground)';
      html += `<div class="pie-item">
        <div class="pie-dot" style="background:${color}"></div>
        <span class="pie-label">${name.replace(/_/g, ' ')}</span>
        <div class="pie-bar-bg"><div class="pie-bar-fill" style="width:${pct}%;background:${color}"></div></div>
        <span class="pie-count">${count} (${pct}%)</span>
      </div>`;
    }
    html += '</div></div>';
  }

  if (bd.by_project) {
    const entries = Object.entries(bd.by_project).sort((a, b) => b[1] - a[1]).slice(0, 10);
    const total = entries.reduce((a, [, b]) => a + b, 0) || 1;
    html += '<div class="pie-card"><h3>Top Projects</h3><div class="pie-items">';
    for (const [name, count] of entries) {
      const pct = (count / total * 100).toFixed(0);
      const label = name.split('/').pop() || name;
      html += `<div class="pie-item">
        <div class="pie-dot" style="background:var(--primary)"></div>
        <span class="pie-label truncate" title="${escapeHtml(name)}">${escapeHtml(label)}</span>
        <div class="pie-bar-bg"><div class="pie-bar-fill" style="width:${pct}%;background:var(--primary);opacity:0.6"></div></div>
        <span class="pie-count">${count}</span>
      </div>`;
    }
    html += '</div></div>';
  }

  html += `<div class="pie-card"><h3>Recent Activity</h3><div class="pie-items" style="gap:16px">
    <div><div class="card-label">Last 7 Days</div><div style="font-size:22px;font-weight:700;margin-top:6px">${bd.recent_7d_sessions || 0} sessions</div><div class="card-sub">${fmtNum(bd.recent_7d_messages || 0)} messages</div></div>
    <div><div class="card-label">Last 30 Days</div><div style="font-size:22px;font-weight:700;margin-top:6px">${bd.recent_30d_sessions || 0} sessions</div><div class="card-sub">${fmtNum(bd.recent_30d_messages || 0)} messages</div></div>
  </div></div>`;

  el.innerHTML = html;
}

// ===== Sessions =====
async function loadSessions() {
  const limit = document.getElementById('sessions-limit').value;
  const body = document.getElementById('sessions-body');
  body.innerHTML = '<tr><td colspan="5" class="loading"><div class="spinner"></div></td></tr>';
  try {
    const data = await api('/sessions?limit=' + limit);
    allSessionsCache = data.sessions || [];
    filterSessions();
  } catch (e) {
    body.innerHTML = `<tr><td colspan="5" class="empty">Error: ${escapeHtml(e.message)}</td></tr>`;
  }
}

function filterSessions() {
  const q = (document.getElementById('sessions-search').value || '').toLowerCase().trim();
  const src = document.getElementById('sessions-source-filter').value;
  let filtered = allSessionsCache;
  if (src) filtered = filtered.filter(s => s.source === src);
  if (q) filtered = filtered.filter(s =>
    (s.project_path || '').toLowerCase().includes(q) ||
    (s.source || '').toLowerCase().includes(q) ||
    (s.id || '').toLowerCase().includes(q)
  );
  renderSessionsTable(filtered);
}

function renderSessionsTable(sessions) {
  const body = document.getElementById('sessions-body');
  if (!sessions.length) {
    body.innerHTML = '<tr><td colspan="5" class="empty">No sessions found</td></tr>';
    document.getElementById('sessions-pagination').innerHTML = '';
    return;
  }
  body.innerHTML = sessions.map(s => `
    <tr onclick="showSessionDetail('${escapeHtml(s.id)}')">
      <td>${badgeFor(s.source)}</td>
      <td class="truncate" style="max-width:220px" title="${escapeHtml(s.project_path || '')}">${escapeHtml((s.project_path || '').split('/').pop() || '-')}</td>
      <td>${(s.message_count || 0).toLocaleString()}</td>
      <td>${relativeTime(s.updated_at)}</td>
      <td style="font-family:monospace;font-size:12px;color:var(--muted-foreground)">${shortId(s.id)}</td>
    </tr>
  `).join('');
  document.getElementById('sessions-pagination').innerHTML =
    `<span class="info">Showing ${sessions.length} sessions</span>`;
}

// ===== Session Detail =====
async function showSessionDetail(id) {
  showPage('session-detail');
  const el = document.getElementById('session-detail-content');
  el.innerHTML = '<div class="loading"><div class="spinner"></div></div>';

  try {
    const [s, summary] = await Promise.all([
      api('/sessions/' + encodeURIComponent(id)),
      api('/sessions/' + encodeURIComponent(id) + '/summary').catch(() => null),
    ]);

    let html = `<div class="detail-header">
      <h2>${badgeFor(s.source)} ${escapeHtml(s.title || (s.project_path || '').split('/').pop() || s.id)}</h2>
      <div class="detail-meta">
        <span>ID: <code>${escapeHtml(s.id)}</code></span>
        <span>Messages: ${(s.message_count || 0).toLocaleString()}</span>
        <span>Created: ${relativeTime(s.created_at)}</span>
        <span>Updated: ${relativeTime(s.updated_at)}</span>
      </div>
    </div>`;

    if (summary && summary.summary) {
      html += '<div class="summary-card"><h3>&#x1F4DD; AI Summary</h3>';
      if (summary.title) html += `<div class="summary-title">${escapeHtml(summary.title)}</div>`;
      html += `<div class="summary-text">${escapeHtml(summary.summary)}</div>`;
      if (summary.topics && summary.topics.length) {
        html += `<div class="summary-topics">${summary.topics.map(t => `<span class="topic-tag">${escapeHtml(t)}</span>`).join('')}</div>`;
      }
      if (summary.decisions && summary.decisions.length) {
        html += `<div class="decisions-list"><strong>Decisions:</strong><ul>${summary.decisions.map(d => `<li>${escapeHtml(d)}</li>`).join('')}</ul></div>`;
      }
      html += '</div>';
    }

    if (s.project_path) {
      html += `<div class="detail-section" style="margin-bottom:16px">
        <span style="font-size:13px;color:var(--muted-foreground)">Project: ${escapeHtml(s.project_path)}</span>
      </div>`;
    }

    if (s.messages && s.messages.length) {
      html += `<div class="detail-section"><h3 style="font-size:13px;color:var(--muted-foreground);margin-bottom:12px;font-weight:600">Messages (${s.messages.length})</h3>`;
      const limit = 100;
      for (const m of s.messages.slice(0, limit)) {
        const role = m.role || 'unknown';
        const cls = role === 'user' ? 'msg-user' : 'msg-assistant';
        const roleColor = role === 'user' ? 'var(--green)' : 'var(--primary)';
        const content = (m.content || '').substring(0, 1200);
        html += `<div class="msg-bubble ${cls}">
          <div class="msg-header">
            <span class="msg-role" style="color:${roleColor}">${role}</span>
            ${m.timestamp ? `<span class="msg-time">${new Date(m.timestamp).toLocaleTimeString()}</span>` : ''}
          </div>${escapeHtml(content)}${content.length >= 1200 ? '...' : ''}
        </div>`;
      }
      if (s.messages.length > limit) {
        html += `<div style="text-align:center;padding:14px;color:var(--muted-foreground);font-size:13px">&hellip; and ${s.messages.length - limit} more messages</div>`;
      }
      html += '</div>';
    }

    el.innerHTML = html;
  } catch (e) {
    el.innerHTML = `<div class="empty">Error: ${escapeHtml(e.message)}</div>`;
  }
}

// ===== Search =====
async function doSearch() {
  const q = document.getElementById('search-input').value.trim();
  if (!q) return;
  const adapter = document.getElementById('search-adapter').value;
  const project = document.getElementById('search-project').value.trim();
  const el = document.getElementById('search-results');
  el.innerHTML = '<div class="loading"><div class="spinner"></div></div>';

  let url = `/search?q=${encodeURIComponent(q)}&limit=50`;
  if (adapter) url += `&adapter=${encodeURIComponent(adapter)}`;
  if (project) url += `&project=${encodeURIComponent(project)}`;

  try {
    const data = await api(url);
    const results = data.results || [];
    if (!results.length) {
      el.innerHTML = `<div class="empty">No results for "${escapeHtml(q)}"</div>`;
      return;
    }
    el.innerHTML = `<div style="font-size:13px;color:var(--muted-foreground);margin-bottom:14px">${results.length} results for "${escapeHtml(q)}"</div>` +
      results.map(r => {
        const snippet = r.snippet || r.content || '';
        return `<div class="search-result" onclick="showSessionDetail('${escapeHtml(r.session_id || '')}')">
          <div class="result-header">
            ${badgeFor(r.adapter || r.source)}
            <span class="truncate" style="color:var(--foreground)">${escapeHtml((r.project || '').split('/').pop() || '-')}</span>
            <span style="color:var(--muted-foreground);margin-left:auto;font-size:11px">${r.chunk_type || ''}</span>
            ${r.timestamp ? `<span style="color:var(--muted-foreground);font-size:11px">${relativeTime(r.timestamp)}</span>` : ''}
          </div>
          <div class="result-snippet">${snippet}</div>
          <div class="result-meta">Session: ${shortId(r.session_id)}</div>
        </div>`;
      }).join('');
  } catch (e) {
    el.innerHTML = `<div class="empty">Error: ${escapeHtml(e.message)}</div>`;
  }
}

// ===== Init =====
document.querySelectorAll('.nav-item').forEach(item => {
  item.addEventListener('click', () => {
    const page = item.dataset.page;
    showPage(page);
    if (page === 'sessions') loadSessions();
    if (page === 'search') document.getElementById('search-input').focus();
  });
});

loadDashboard();
