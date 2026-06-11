#!/usr/bin/env python3
"""Build the Memex docs site from Markdown sources.

Inputs (auto-discovered):
  - README.md             → index.html
  - SKILL.md              → SKILL.html
  - design/**/*.md        → design/<slug>.html   (递归扫 specs/todos/decisions/prototypes)
  - skills/*/SKILL.md     → skills/<ide>.html
  - design/**/*.html      → 原样 copy 到 site/design/<basename>.html

  注：design 输出目录始终扁平（不复制子目录结构），slug 取自文件名，
  跨子目录的文件名不能重名。

Output: site/  (用于 GitHub Pages artifact)

Design goals:
  - 零脚手架：单文件、单模板、Markdown + Pygments 高亮
  - 自动 sidebar：按文档类型分组（README / Design Docs / Skills）
  - 不引入 Hugo/mdBook 等大型 SSG，避免维护成本
"""

from __future__ import annotations

import re
import shutil
from html import escape
from pathlib import Path

import markdown

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "site"

# ---------------------------------------------------------------------------
# 模板
# ---------------------------------------------------------------------------

HTML_SHELL = """<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Memex Docs</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{ margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", "PingFang SC", "Microsoft YaHei", sans-serif;
          background: #fafafa; color: #1a1a1a; line-height: 1.65; }}
  @media (prefers-color-scheme: dark) {{ body {{ background: #0f0f10; color: #e8e8e8; }} }}
  .layout {{ display: grid; grid-template-columns: 280px 1fr; min-height: 100vh; }}
  .sidebar {{ position: sticky; top: 0; height: 100vh; overflow-y: auto;
             padding: 24px 16px; border-right: 1px solid rgba(0,0,0,.08);
             background: rgba(255,255,255,.6); backdrop-filter: blur(6px); }}
  @media (prefers-color-scheme: dark) {{
    .sidebar {{ background: rgba(20,20,22,.65); border-right-color: rgba(255,255,255,.08); }}
  }}
  .brand {{ font-weight: 700; font-size: 18px; margin: 0 8px 18px; }}
  .brand a {{ color: inherit; text-decoration: none; }}
  .nav-group h3 {{ font-size: 11px; text-transform: uppercase; letter-spacing: .08em;
                  margin: 18px 8px 6px; color: #888; }}
  .nav-group ul {{ list-style: none; padding: 0; margin: 0; }}
  .nav-group li a {{ display: block; padding: 6px 8px; border-radius: 6px;
                    color: inherit; text-decoration: none; font-size: 14px; }}
  .nav-group li a:hover {{ background: rgba(0,0,0,.05); }}
  .nav-group li a.active {{ background: #2563eb; color: #fff; }}
  @media (prefers-color-scheme: dark) {{
    .nav-group li a:hover {{ background: rgba(255,255,255,.06); }}
  }}
  main {{ padding: 48px 60px; max-width: 920px; }}
  main h1 {{ margin-top: 0; }}
  main pre {{ background: #1e1e1e; color: #e4e4e4; padding: 12px 16px;
             border-radius: 6px; overflow-x: auto; font-size: 13px; }}
  main code {{ font-family: "SF Mono", Menlo, Consolas, monospace; font-size: .92em; }}
  main p > code, main li > code, main td > code {{ background: rgba(0,0,0,.05);
        padding: 1px 6px; border-radius: 3px; }}
  @media (prefers-color-scheme: dark) {{ main p > code, main li > code, main td > code {{ background: rgba(255,255,255,.08); }} }}
  main table {{ border-collapse: collapse; margin: 16px 0; }}
  main th, main td {{ border: 1px solid rgba(0,0,0,.12); padding: 6px 12px; }}
  @media (prefers-color-scheme: dark) {{ main th, main td {{ border-color: rgba(255,255,255,.15); }} }}
  main blockquote {{ margin: 16px 0; padding: 8px 16px;
                    border-left: 4px solid #2563eb; background: rgba(37,99,235,.06); }}
  main a {{ color: #2563eb; }}
  @media (prefers-color-scheme: dark) {{ main a {{ color: #60a5fa; }} }}
  @media (max-width: 800px) {{
    .layout {{ grid-template-columns: 1fr; }}
    .sidebar {{ position: static; height: auto; }}
    main {{ padding: 24px 20px; }}
  }}
  {pygments_css}
</style>
</head>
<body>
<div class="layout">
  <aside class="sidebar">
    <div class="brand"><a href="{root_href}">Memex Docs</a></div>
    {nav}
  </aside>
  <main>
    {content}
  </main>
</div>
</body>
</html>
"""


# ---------------------------------------------------------------------------
# Markdown 渲染
# ---------------------------------------------------------------------------

MD = markdown.Markdown(
    extensions=[
        "fenced_code",
        "codehilite",
        "tables",
        "toc",
        "sane_lists",
        "admonition",
    ],
    extension_configs={
        "codehilite": {"guess_lang": False, "noclasses": False},
        "toc": {"permalink": True},
    },
)


def render_markdown(text: str) -> str:
    MD.reset()
    return MD.convert(text)


def first_h1(text: str, fallback: str) -> str:
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


# ---------------------------------------------------------------------------
# Site builder
# ---------------------------------------------------------------------------


def slugify(name: str) -> str:
    name = re.sub(r"\.(md|html)$", "", name)
    return name.replace("/", "-")


def collect_pages() -> tuple[list[dict], list[dict], list[dict]]:
    """Return (root_pages, design_pages, skill_pages)."""
    root_pages: list[dict] = []
    for fn in ("README.md", "SKILL.md"):
        p = ROOT / fn
        if not p.exists():
            continue
        slug = "index" if fn == "README.md" else slugify(fn)
        text = p.read_text(encoding="utf-8")
        root_pages.append(
            {
                "slug": slug,
                "title": "Memex" if fn == "README.md" else "Project SKILL",
                "src": p,
                "out": OUT / f"{slug}.html",
                "href": f"{slug}.html",
                "text": text,
            }
        )

    design_pages: list[dict] = []
    for p in sorted((ROOT / "design").rglob("*.md")):
        slug = slugify(p.name)
        text = p.read_text(encoding="utf-8")
        design_pages.append(
            {
                "slug": slug,
                "title": first_h1(text, p.stem),
                "src": p,
                "out": OUT / "design" / f"{slug}.html",
                "href": f"design/{slug}.html",
                "text": text,
            }
        )

    skill_pages: list[dict] = []
    for p in sorted((ROOT / "skills").glob("*/SKILL.md")):
        ide = p.parent.name
        text = p.read_text(encoding="utf-8")
        skill_pages.append(
            {
                "slug": ide,
                "title": f"SKILL: {ide}",
                "src": p,
                "out": OUT / "skills" / f"{ide}.html",
                "href": f"skills/{ide}.html",
                "text": text,
            }
        )

    return root_pages, design_pages, skill_pages


def render_nav(active_href: str, groups: list[tuple[str, list[dict]]], depth_to_root: int) -> str:
    prefix = "../" * depth_to_root
    out = []
    for title, pages in groups:
        if not pages:
            continue
        out.append('<div class="nav-group">')
        out.append(f"<h3>{escape(title)}</h3><ul>")
        for pg in pages:
            cls = "active" if pg["href"] == active_href else ""
            out.append(
                f'<li><a href="{prefix}{pg["href"]}" class="{cls}">{escape(pg["title"])}</a></li>'
            )
        out.append("</ul></div>")
    return "\n".join(out)


def main() -> None:
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir()

    pygments_css = ""
    try:
        from pygments.formatters import HtmlFormatter

        pygments_css = HtmlFormatter(style="monokai").get_style_defs(".codehilite")
    except Exception:  # noqa: BLE001
        pass

    root_pages, design_pages, skill_pages = collect_pages()
    groups = [
        ("Overview", root_pages),
        ("Design Docs", design_pages),
        ("IDE Skills", skill_pages),
    ]

    for pg in root_pages + design_pages + skill_pages:
        pg["out"].parent.mkdir(parents=True, exist_ok=True)
        body_html = render_markdown(pg["text"])
        depth = len(pg["out"].relative_to(OUT).parts) - 1
        nav = render_nav(pg["href"], groups, depth)
        root_href = "../" * depth + "index.html"
        html = HTML_SHELL.format(
            title=escape(pg["title"]),
            pygments_css=pygments_css,
            root_href=root_href,
            nav=nav,
            content=body_html,
        )
        pg["out"].write_text(html, encoding="utf-8")
        print(f"[docs] {pg['src'].relative_to(ROOT)} → {pg['out'].relative_to(ROOT)}")

    design_html_src = ROOT / "design"
    for p in design_html_src.rglob("*.html"):
        target = OUT / "design" / p.name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, target)
        print(f"[docs] copy {p.relative_to(ROOT)} → {target.relative_to(ROOT)}")

    print(f"\n[docs] done → {OUT}")


if __name__ == "__main__":
    main()
