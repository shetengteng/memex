//! 沙箱内 markdown 文件的遍历与过滤。
//!
//! 把"扫描所有 session md 文件、按 adapter / project / 时间窗过滤"这一公共能力
//! 抽出来，供 [`super::grep`] / [`super::find`] / [`super::read`] 复用。

use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use super::frontmatter::{Frontmatter, adapter_from_path, parse_frontmatter};
use super::sandbox::sessions_root;

pub(super) struct ScanFilter {
    pub adapter: Option<String>,
    pub project: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

/// 把 `YYYY-MM-DD` 或 RFC3339 字符串解析为 UTC 时间。
pub(super) fn parse_date_loose(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(DateTime::<Utc>::from_naive_utc_and_offset(
            d.and_hms_opt(0, 0, 0)?,
            Utc,
        ));
    }
    None
}

/// 读取文件 mtime 并转成 UTC。无法读取时返回 None。
pub(super) fn mtime_of(path: &Path) -> Option<DateTime<Utc>> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    Some(DateTime::<Utc>::from(modified))
}

fn in_time_window(
    mtime: Option<DateTime<Utc>>,
    after: Option<&DateTime<Utc>>,
    before: Option<&DateTime<Utc>>,
) -> bool {
    if after.is_none() && before.is_none() {
        return true;
    }
    let Some(t) = mtime else {
        return false;
    };
    if let Some(a) = after
        && t < *a
    {
        return false;
    }
    if let Some(b) = before
        && t > *b
    {
        return false;
    }
    true
}

/// 用入参字符串构造一个 [`ScanFilter`]。
///
/// # Errors
///
/// `after` / `before` 字符串不是合法 ISO 日期时报错。
pub(super) fn parse_filter(
    adapter: &Option<String>,
    project: &Option<String>,
    after: &Option<String>,
    before: &Option<String>,
) -> Result<ScanFilter> {
    let after_dt = match after {
        Some(s) => Some(parse_date_loose(s).ok_or_else(|| anyhow!("invalid 'after' date: {}", s))?),
        None => None,
    };
    let before_dt = match before {
        Some(s) => {
            Some(parse_date_loose(s).ok_or_else(|| anyhow!("invalid 'before' date: {}", s))?)
        }
        None => None,
    };
    Ok(ScanFilter {
        adapter: adapter.clone(),
        project: project.clone(),
        after: after_dt,
        before: before_dt,
    })
}

/// 遍历沙箱内所有 md 文件，对每个文件调用 `visit`。回调返回 false 时停止遍历。
pub(super) fn for_each_session<F>(filter: &ScanFilter, mut visit: F) -> Result<()>
where
    F: FnMut(&Path, &Frontmatter, &fs::Metadata) -> bool,
{
    let root = sessions_root();
    if !root.exists() {
        return Ok(());
    }
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());
    for entry in walker {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let adapter = adapter_from_path(path);
        if let Some(ref want) = filter.adapter
            && adapter.as_deref() != Some(want.as_str())
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let mtime = meta.modified().ok().map(DateTime::<Utc>::from);
        if !in_time_window(mtime, filter.after.as_ref(), filter.before.as_ref()) {
            continue;
        }
        let fm = parse_frontmatter(path);
        if let Some(ref want) = filter.project {
            let lower = want.to_lowercase();
            let matches = fm
                .project
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains(&lower));
            if !matches {
                continue;
            }
        }
        if !visit(path, &fm, &meta) {
            break;
        }
    }
    Ok(())
}
