//! `memex search` —— Phase 5a 起完全走 daemon RPC，不再 fallback 到本地 db。
//!
//! 设计：`MemexClient::connect` 已在 client.rs 内做"主进程未起 → 友好报错"，
//! 这里只负责拼 query string 和 print。

use std::time::Instant;

use anyhow::Result;

use crate::client::MemexClient;

#[allow(clippy::too_many_arguments)]
pub fn run(
    query: &str,
    limit: usize,
    json: bool,
    adapter: Option<String>,
    project: Option<String>,
    chunk_type: Option<String>,
    after: Option<String>,
    before: Option<String>,
) -> Result<()> {
    let client = MemexClient::connect()?;

    let mut params = format!("/search?q={}&limit={}", urlenc(query), limit);
    if let Some(a) = &adapter {
        params.push_str(&format!("&adapter={}", urlenc(a)));
    }
    if let Some(p) = &project {
        params.push_str(&format!("&project={}", urlenc(p)));
    }
    if let Some(c) = &chunk_type {
        params.push_str(&format!("&chunk_type={}", urlenc(c)));
    }
    if let Some(a) = &after {
        params.push_str(&format!("&after={}", urlenc(a)));
    }
    if let Some(b) = &before {
        params.push_str(&format!("&before={}", urlenc(b)));
    }

    let started = Instant::now();
    let body = client.get_value(&params)?;
    let latency = started.elapsed().as_millis();

    if json {
        crate::io::json(&body)?;
        return Ok(());
    }

    let Some(results) = body.get("results").and_then(|v| v.as_array()) else {
        crate::out!("No results for \"{}\"", query);
        return Ok(());
    };
    if results.is_empty() {
        crate::out!("No results for \"{}\"", query);
        return Ok(());
    }
    crate::out!(
        "Found {} result(s) for \"{}\" ({} ms):\n",
        results.len(),
        query,
        latency
    );
    for (i, r) in results.iter().enumerate() {
        let sid = r.get("session_id").and_then(|v| v.as_str()).unwrap_or("?");
        let prefix = &sid[..8.min(sid.len())];
        let ct = r.get("chunk_type").and_then(|v| v.as_str()).unwrap_or("?");
        let src = r.get("adapter").and_then(|v| v.as_str()).unwrap_or("?");
        let snip = r.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
        let reason = r.get("match_reason").and_then(|v| v.as_str()).unwrap_or("");
        crate::out!("{}. [{}] session:{} ({})", i + 1, ct, prefix, src);
        crate::out!("   {}", snip.replace('\n', " "));
        crate::out!("   reason: {}\n", reason);
    }

    Ok(())
}

/// 极简 URL-encoder：只处理 query string 里有歧义的三个字符。
/// 用 form-urlencoded 全套库（如 `url`、`percent-encoding`）会拖一堆依赖；
/// memex 的 search query 大都是英文 + 中文 + 空格，覆盖这三个 ASCII 已经够。
fn urlenc(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlenc_escapes_query_separators() {
        assert_eq!(urlenc("redis cache"), "redis%20cache");
        assert_eq!(urlenc("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn urlenc_preserves_non_separator_characters() {
        assert_eq!(urlenc("ZOOM-123 中文/path"), "ZOOM-123%20中文/path");
    }
}
