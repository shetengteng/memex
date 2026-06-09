use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub latest_tag: String,
    pub html_url: String,
}

const ATOM_URL: &str = "https://github.com/shetengteng/memex/releases.atom";
const RELEASES_PAGE: &str = "https://github.com/shetengteng/memex/releases";
const TAG_MARKER: &str = "tag:github.com,2008:Repository/";

fn parse_latest_tag(xml: &str) -> Option<String> {
    let entry_start = xml.find("<entry")?;
    let entry_slice = &xml[entry_start..];
    let id_start = entry_slice.find("<id>")?;
    let id_end = entry_slice.find("</id>")?;
    if id_end <= id_start {
        return None;
    }
    let id_value = &entry_slice[id_start + "<id>".len()..id_end];
    let marker_pos = id_value.find(TAG_MARKER)?;
    let after_marker = &id_value[marker_pos + TAG_MARKER.len()..];
    let slash_pos = after_marker.find('/')?;
    let tag = after_marker[slash_pos + 1..].trim();
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let body = tauri::async_runtime::spawn_blocking(|| -> Result<String, String> {
        let mut resp = ureq::get(ATOM_URL)
            .header("Accept", "application/atom+xml")
            .header("User-Agent", "memex-menubar")
            .call()
            .map_err(|e| format!("request failed: {e}"))?;
        if resp.status() != 200 {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.body_mut()
            .read_to_string()
            .map_err(|e| format!("read body failed: {e}"))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    let tag = parse_latest_tag(&body).ok_or_else(|| "no release entry in feed".to_string())?;
    let html_url = format!("{RELEASES_PAGE}/tag/{tag}");
    Ok(UpdateInfo {
        latest_tag: tag,
        html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_latest_tag_extracts_first_entry_tag() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>tag:github.com,2008:https://github.com/shetengteng/memex/releases</id>
  <entry>
    <id>tag:github.com,2008:Repository/1255222746/v0.2.1</id>
    <title>v0.2.1</title>
  </entry>
  <entry>
    <id>tag:github.com,2008:Repository/1255222746/v0.2.0</id>
    <title>v0.2.0</title>
  </entry>
</feed>"#;
        assert_eq!(parse_latest_tag(xml), Some("v0.2.1".to_string()));
    }

    #[test]
    fn parse_latest_tag_returns_none_when_no_entry() {
        let xml = r#"<feed><id>tag:github.com,2008:foo</id></feed>"#;
        assert_eq!(parse_latest_tag(xml), None);
    }
}
