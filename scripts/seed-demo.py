#!/usr/bin/env python3
"""seed-demo.py — 给 ~/.memex-demo/memex.db 灌入虚构的 mock 数据。

设计目标：
* 完全虚构的 user / projects / IDE 路径，无任何隐私数据
* 数字看起来"活"（Today / Library / Insights 各页面都有内容）
* 数据时间戳锚定"今天"，让 KPI / 趋势图渲染合理
* 不写 chunks_fts（trigger 自动同步）
* 5 个虚构项目 × 5 个 adapter，匹配 Connect 页的适配器开关

跑法:
    rm ~/.memex-demo/memex.db*  # 如果想完全重置
    USER="Alex Chen" MEMEX_HOME=~/.memex-demo /Applications/Memex.app/Contents/MacOS/Memex &
    sleep 6 && kill <pid>  # 让 migrations 跑完
    python3 scripts/seed-demo.py
"""

import hashlib
import os
import random
import sqlite3
from datetime import datetime, timedelta, timezone
from pathlib import Path

random.seed(42)

DB_PATH = Path.home() / ".memex-demo" / "memex.db"
NOW = datetime.now(timezone.utc)
WEEK_AGO = NOW - timedelta(days=7)
MONTH_AGO = NOW - timedelta(days=30)

# --- 虚构 project + adapter 分布 ---
PROJECTS = [
    ("/Users/alex/Code/acme-platform", "acme-platform"),
    ("/Users/alex/Code/orbit-design-system", "orbit-design-system"),
    ("/Users/alex/Code/nimbus-cli", "nimbus-cli"),
    ("/Users/alex/Code/pulse-analytics", "pulse-analytics"),
    ("/Users/alex/Code/kepler-mobile", "kepler-mobile"),
]
ADAPTERS = ["cursor", "claude_code", "codex", "opencode", "continue_dev"]
ADAPTER_WEIGHTS = [0.50, 0.22, 0.12, 0.10, 0.06]

# --- 虚构对话内容片段 ---
TOPICS = [
    "Refactor the user authentication flow to use JWT",
    "Add Redis caching layer for the search endpoint",
    "Fix race condition in the websocket reconnect logic",
    "Design schema for the audit log table",
    "Optimize SQL query in the dashboard aggregator",
    "Set up CI pipeline for the mobile builds",
    "Implement dark mode toggle in the design system",
    "Migrate from Express to Fastify for the gateway",
    "Wire up the OpenAPI generator to the CI",
    "Replace lodash with native ES utilities",
    "Add E2E tests for the checkout flow",
    "Document the new theming tokens",
    "Profile memory leak in the long-running worker",
    "Polish empty-state illustrations for the dashboard",
    "Roll out feature flags for the new pricing page",
    "Hook up Sentry to the React Native shell",
    "Set up shadcn primitives for the form library",
    "Tighten TypeScript strictness in the shared package",
    "Reduce bundle size by tree-shaking icon imports",
    "Plan the data migration for the v3 release",
]
USER_MESSAGES = [
    "Can you outline the steps to ship this safely?",
    "Walk me through the trade-offs vs the alternative.",
    "I want a minimal diff that keeps existing behavior.",
    "Show me only the files that need to change.",
    "Why did you pick this pattern over the existing one?",
    "Run the tests and report what fails.",
    "Add a regression test that covers the bug.",
    "Refactor without changing public API surface.",
]
ASSISTANT_MESSAGES = [
    "Here is the plan: 1) extract the helper into its own module, 2) thread it through the call sites, 3) add a focused test. Want me to start with step 1?",
    "I'd lean toward Option B — it removes a layer of indirection and the perf delta is within noise. The migration is mechanical, no behavior change.",
    "Three files need to change: src/auth/session.ts (new helper), src/middleware/index.ts (wire it in), and the corresponding test. Diff is ~40 lines.",
    "I dropped the implicit `any` from the public interface and propagated the strict signature. CI should be green now.",
    "The root cause was a missing cleanup in the unmount path. Patched and added a guard test. PR ready.",
    "I see one risk: the cache key collides on tenant ID across regions. Suggest namespacing it as `region:tenant:user`.",
    "Looks good to ship. Tests pass locally, lint clean, no behavior change visible in the e2e flow.",
]

def conn():
    return sqlite3.connect(DB_PATH)

def random_ts_in(start: datetime, end: datetime) -> datetime:
    """均匀分布在区间内的随机时间戳。"""
    delta_s = int((end - start).total_seconds())
    return start + timedelta(seconds=random.randint(0, delta_s))

def hash_content(content: str) -> str:
    return hashlib.sha256(content.encode("utf-8")).hexdigest()[:16]

def insert_sources(c: sqlite3.Cursor) -> None:
    """虚构的 source 路径，让 Connect 页的"采集源"卡片渲染出"已扫描多少文件"。
    用 /Users/alex 前缀避免任何真实路径泄露。
    """
    fake_paths_per_adapter = {
        "cursor": [
            f"/Users/alex/Library/Application Support/Cursor/User/workspaceStorage/acme-{i:08x}/state.vscdb"
            for i in range(2530)
        ][:50],  # 只插 50 行做指示，count 字段是另一回事
        "claude_code": [
            f"/Users/alex/.claude/projects/acme-platform/{i:032x}.jsonl" for i in range(255)
        ][:50],
        "codex": [f"/Users/alex/.codex/sessions/{i:08x}.json" for i in range(13)],
        "opencode": [
            f"/Users/alex/.local/share/opencode/projects/acme/{i:08x}.jsonl" for i in range(124)
        ][:30],
        "continue_dev": [f"/Users/alex/.continue/sessions/{i:08x}.json" for i in range(1)],
    }
    for adapter, paths in fake_paths_per_adapter.items():
        for p in paths:
            c.execute(
                "INSERT INTO sources (adapter, file_path, last_offset, last_mtime, last_scan) VALUES (?, ?, ?, ?, ?)",
                (adapter, p, 0, int(NOW.timestamp()), NOW.isoformat()),
            )

def choose_adapter() -> str:
    r = random.random()
    cum = 0.0
    for a, w in zip(ADAPTERS, ADAPTER_WEIGHTS):
        cum += w
        if r < cum:
            return a
    return ADAPTERS[-1]

def insert_sessions_and_messages(c: sqlite3.Cursor) -> None:
    """虚构 sessions + messages + chunks。

    分布：今天 12 / 本周累计 87 / 本月累计 ~250。Today 页 KPI 看起来活生生。
    """
    today_start = NOW.replace(hour=0, minute=0, second=0, microsecond=0)

    # session 分布（按时间分桶让 KPI 数字稳定）
    buckets = [
        (today_start, NOW, 12),                                 # 今天 12
        (today_start - timedelta(days=1), today_start, 18),     # 昨天
        (WEEK_AGO + timedelta(days=1), today_start - timedelta(days=1), 57),  # 本周剩余
        (MONTH_AGO + timedelta(days=2), WEEK_AGO, 163),         # 本月剩余
    ]

    session_idx = 0
    summary_candidates = []
    for start, end, count in buckets:
        for _ in range(count):
            project_path, project_label = random.choice(PROJECTS)
            adapter = choose_adapter()
            session_id = f"sess_{session_idx:06x}_{adapter[:3]}"
            created = random_ts_in(start, end)
            updated = created + timedelta(minutes=random.randint(2, 90))

            topic = random.choice(TOPICS)
            file_path = f"{project_path}/.{adapter}/sessions/{session_id}.jsonl"

            c.execute(
                """INSERT INTO sessions
                   (id, source, project_path, file_path, title, created_at, updated_at, message_count, intent)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (session_id, adapter, project_path, file_path, topic,
                 created.isoformat(), updated.isoformat(), 0,
                 topic if random.random() < 0.6 else None),
            )

            msg_count = random.randint(3, 32)
            for m_idx in range(msg_count):
                role = "user" if m_idx % 2 == 0 else "assistant"
                content = random.choice(USER_MESSAGES if role == "user" else ASSISTANT_MESSAGES)
                msg_id = f"msg_{session_idx:06x}_{m_idx:03x}"
                msg_ts = created + timedelta(seconds=m_idx * random.randint(20, 300))
                c.execute(
                    """INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
                       VALUES (?, ?, ?, ?, ?, ?, ?)""",
                    (msg_id, session_id, role, content, msg_ts.isoformat(), m_idx * 256, hash_content(content + str(m_idx))),
                )
                c.execute(
                    """INSERT INTO chunks (message_id, session_id, chunk_type, content, position, token_count, metadata_json)
                       VALUES (?, ?, 'text', ?, ?, ?, '{}')""",
                    (msg_id, session_id, content, m_idx, len(content) // 4),
                )

            c.execute("UPDATE sessions SET message_count = ? WHERE id = ?", (msg_count, session_id))

            if random.random() < 0.15:
                summary_candidates.append((session_id, topic, msg_count, updated))

            session_idx += 1

    # L2 summaries —— Library 的"已摘要" filter 才有 hits
    for sid, topic, mcount, updated in summary_candidates[:38]:
        summary = (
            f"Worked on {topic.lower()}. Decided to take a phased approach, "
            f"starting with the smallest reversible diff. Identified two follow-ups for next session."
        )
        c.execute(
            """INSERT INTO summaries
               (session_id, level, title, summary, topics_json, decisions_json, created_at, message_count_at_creation)
               VALUES (?, 'L2_session', ?, ?, '[]', '[]', ?, ?)""",
            (sid, topic, summary, updated.isoformat(), mcount),
        )

def insert_metrics(c: sqlite3.Cursor) -> None:
    """日维度统计数据，给 Insights 趋势图。"""
    for d in range(30):
        date = (NOW - timedelta(days=d)).date().isoformat()
        for name, base in [("sessions_total", 8), ("messages_total", 180), ("mcp_calls", 4)]:
            c.execute(
                "INSERT INTO metrics (date, metric_name, metric_value) VALUES (?, ?, ?)",
                (date, name, base + random.randint(0, base // 2)),
            )

def insert_access_log(c: sqlite3.Cursor) -> None:
    queries = [
        "redis caching", "jwt", "websocket reconnect", "audit log schema",
        "dashboard query plan", "shadcn primitives", "tree-shake icons",
        "feature flag rollout", "OpenAPI generator", "dark mode tokens",
    ]
    for d in range(60):
        ts = NOW - timedelta(hours=random.randint(0, 24 * 30))
        c.execute(
            "INSERT INTO access_log (query, result_count, latency_ms, created_at) VALUES (?, ?, ?, ?)",
            (random.choice(queries), random.randint(2, 40), random.randint(8, 220), ts.isoformat()),
        )

def insert_kv(c: sqlite3.Cursor) -> None:
    pairs = [
        ("ui.locale", "zh"),
        ("ui.theme", "light"),
        ("pref.privacy.auto_redact", "true"),
        ("pref.privacy.private_from_mcp", "false"),
        ("pref.notify.weekly_report", "true"),
        ("pref.notify.reflect_pending", "true"),
        ("pref.notify.ingest_failed", "true"),
    ]
    for k, v in pairs:
        c.execute(
            "INSERT INTO kv (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (k, v),
        )

def insert_llm_providers(c: sqlite3.Cursor) -> None:
    c.execute(
        """INSERT INTO llm_providers
           (id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at)
           VALUES ('ollama_local', 'Ollama Local', 'ollama', 'http://127.0.0.1:11434', 'qwen2.5:7b', '', 1, 1, 'ok', 1742, ?)""",
        (NOW.isoformat(),),
    )

def insert_mcp_log(c: sqlite3.Cursor) -> None:
    tools = ["search_memory", "get_project_context", "list_recent", "stats", "list_sessions_by_range", "get_session"]
    for _ in range(48):
        ts = NOW - timedelta(hours=random.randint(0, 24))
        c.execute(
            "INSERT INTO mcp_call_log (occurred_at, tool_name, latency_ms, success) VALUES (?, ?, ?, 1)",
            (ts.isoformat(), random.choice(tools), random.randint(40, 600)),
        )

def main() -> None:
    if not DB_PATH.exists():
        raise SystemExit(f"db not found: {DB_PATH}. Bootstrap Memex with MEMEX_HOME=~/.memex-demo first.")

    db = conn()
    try:
        c = db.cursor()
        # 清空（除非全空）再插 —— seed 脚本幂等
        for tbl in ("chunks", "messages", "sessions", "summaries", "aggregate_summaries",
                    "sources", "metrics", "access_log", "kv", "llm_providers", "mcp_call_log",
                    "notifications", "thread_sessions", "threads"):
            c.execute(f"DELETE FROM {tbl}")
        c.execute("DELETE FROM chunks_fts")

        insert_sources(c)
        insert_sessions_and_messages(c)
        insert_metrics(c)
        insert_access_log(c)
        insert_kv(c)
        insert_llm_providers(c)
        insert_mcp_log(c)

        # FTS 重建（trigger 应该已经同步，但保险起见）
        c.execute("INSERT INTO chunks_fts(chunks_fts) VALUES('rebuild')")

        db.commit()

        # 总结
        for q in [
            "SELECT 'sessions=' || COUNT(*) FROM sessions",
            "SELECT 'messages=' || COUNT(*) FROM messages",
            "SELECT 'chunks=' || COUNT(*) FROM chunks",
            "SELECT 'summaries=' || COUNT(*) FROM summaries",
        ]:
            row = c.execute(q).fetchone()
            print(row[0])
    finally:
        db.close()

if __name__ == "__main__":
    main()
