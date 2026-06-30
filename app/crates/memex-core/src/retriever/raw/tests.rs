//! 16 个端到端单测，覆盖 raw_grep / raw_find / raw_read 的命中路径、过滤路径与
//! 错误路径。
//!
//! **必须以 `--test-threads=1` 运行**：所有用例通过 `MEMEX_HOME` 重定向沙箱根，
//! 这是进程级环境变量，并行执行会互相覆盖。

use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::Path;

use tempfile::TempDir;

use super::{
    RawFindRequest, RawGrepRequest, RawReadRequest, raw_find, raw_grep, raw_read,
};

fn setup_sandbox() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    // INVARIANT: 测试入口已串行化，进程级 env 变量不会并发竞争
    unsafe {
        std::env::set_var("MEMEX_HOME", tmp.path());
    }
    std::fs::create_dir_all(tmp.path().join("sessions/claude_code")).expect("mkdir");
    tmp
}

fn write_session(dir: &Path, adapter: &str, sid: &str, project: Option<&str>, body: &str) {
    let path = dir
        .join("sessions")
        .join(adapter)
        .join(format!("{}.md", sid));
    std::fs::create_dir_all(path.parent().expect("has parent")).expect("mkdir");
    let mut f = File::create(&path).expect("create");
    writeln!(f, "---").expect("write");
    writeln!(f, "session_id: {}", sid).expect("write");
    writeln!(f, "source: {}", adapter).expect("write");
    if let Some(p) = project {
        writeln!(f, "project: {}", p).expect("write");
    }
    writeln!(f, "---").expect("write");
    writeln!(f).expect("write");
    f.write_all(body.as_bytes()).expect("write body");
}

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).expect("non-zero")
}

#[test]
fn grep_finds_literal_match() {
    let tmp = setup_sandbox();
    write_session(
        tmp.path(),
        "claude_code",
        "sess-a",
        Some("/proj/a"),
        "line one\nredis pipeline error\nline three\n",
    );
    let resp = raw_grep(RawGrepRequest {
        query: "redis pipeline".to_string(),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
    let hit = &resp.hits[0];
    assert_eq!(hit.session_id, "sess-a");
    assert_eq!(hit.adapter, "claude_code");
    assert!(hit.snippet.contains("redis pipeline"));
    assert_eq!(hit.deep_link, "memex://session/sess-a");
}

#[test]
fn grep_respects_adapter_filter() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "s1", None, "hello world");
    write_session(tmp.path(), "codex", "s2", None, "hello world");
    let resp = raw_grep(RawGrepRequest {
        query: "hello".to_string(),
        adapter: Some("codex".to_string()),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
    assert_eq!(resp.hits[0].adapter, "codex");
}

#[test]
fn grep_files_only_dedups() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "s1", None, "foo\nfoo\nfoo\n");
    let resp = raw_grep(RawGrepRequest {
        query: "foo".to_string(),
        files_only: true,
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
}

#[test]
fn grep_regex_mode() {
    let tmp = setup_sandbox();
    write_session(
        tmp.path(),
        "claude_code",
        "s1",
        None,
        "version 1.2.3 released\n",
    );
    let resp = raw_grep(RawGrepRequest {
        query: r"\d+\.\d+\.\d+".to_string(),
        regex: true,
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
}

#[test]
fn grep_invalid_regex_errors() {
    let _tmp = setup_sandbox();
    let err = raw_grep(RawGrepRequest {
        query: "(".to_string(),
        regex: true,
        ..Default::default()
    })
    .expect_err("should fail");
    assert!(err.to_string().contains("invalid_regex"));
}

#[test]
fn grep_case_sensitive_toggle() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "s1", None, "Redis Pipeline\n");
    let resp = raw_grep(RawGrepRequest {
        query: "redis pipeline".to_string(),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
    let resp2 = raw_grep(RawGrepRequest {
        query: "redis pipeline".to_string(),
        case_sensitive: true,
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp2.hits.len(), 0);
}

#[test]
fn grep_truncated_when_internal_cap_hit() {
    let tmp = setup_sandbox();
    // limit=2, internal_cap = 2*5 = 10。写 12 个全部命中的文件，确保触发截断。
    let body = "match-this-line\n";
    for i in 0..12 {
        write_session(tmp.path(), "claude_code", &format!("s{:02}", i), None, body);
    }
    let resp = raw_grep(RawGrepRequest {
        query: "match-this-line".to_string(),
        limit: 2,
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 2);
    assert!(resp.truncated, "should be truncated when more matches exist");
}

#[test]
fn find_by_name_pattern() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "abc-1", None, "x");
    write_session(tmp.path(), "claude_code", "xyz-9", None, "x");
    let resp = raw_find(RawFindRequest {
        name_pattern: Some("abc*".to_string()),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.files.len(), 1);
    assert_eq!(resp.files[0].session_id, "abc-1");
}

#[test]
fn find_by_project_filter() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "s1", Some("/proj/alpha"), "x");
    write_session(tmp.path(), "claude_code", "s2", Some("/proj/beta"), "x");
    let resp = raw_find(RawFindRequest {
        project: Some("alpha".to_string()),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.files.len(), 1);
    assert_eq!(resp.files[0].session_id, "s1");
}

#[test]
fn find_time_window_filters_by_mtime() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "now", None, "x");
    let resp = raw_find(RawFindRequest {
        before: Some("2000-01-01".to_string()),
        ..Default::default()
    })
    .expect("ok");
    assert!(resp.files.is_empty(), "expected 0 hits, got {:?}", resp.files);
    let resp = raw_find(RawFindRequest {
        after: Some("2000-01-01".to_string()),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.files.len(), 1);
}

#[test]
fn find_invalid_date_errors() {
    let _tmp = setup_sandbox();
    let err = raw_find(RawFindRequest {
        after: Some("not-a-date".to_string()),
        ..Default::default()
    })
    .expect_err("should fail");
    assert!(err.to_string().contains("invalid 'after' date"));
}

#[test]
fn find_size_filters() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "tiny", None, "x");
    write_session(
        tmp.path(),
        "claude_code",
        "big",
        None,
        &"y".repeat(3 * 1024),
    );
    let resp = raw_find(RawFindRequest {
        min_size_kb: 2,
        ..Default::default()
    })
    .expect("ok");
    let ids: Vec<&str> = resp.files.iter().map(|f| f.session_id.as_str()).collect();
    assert!(ids.contains(&"big"));
    assert!(!ids.contains(&"tiny"));
}

#[test]
fn frontmatter_missing_falls_back_to_filename() {
    let tmp = setup_sandbox();
    let path = tmp
        .path()
        .join("sessions/claude_code/bare-sid.md");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "just some plain content with redis\n").expect("write");
    let resp = raw_grep(RawGrepRequest {
        query: "redis".to_string(),
        ..Default::default()
    })
    .expect("ok");
    assert_eq!(resp.hits.len(), 1);
    assert_eq!(resp.hits[0].session_id, "bare-sid");
    assert_eq!(resp.hits[0].adapter, "claude_code");
}

#[test]
fn read_by_session_id() {
    let tmp = setup_sandbox();
    write_session(tmp.path(), "claude_code", "sess-r", None, "a\nb\nc\nd\ne\n");
    let resp = raw_read(RawReadRequest {
        session_id: Some("sess-r".to_string()),
        file: None,
        start_line: nz(1),
        end_line: nz(3),
    })
    .expect("ok");
    assert_eq!(resp.lines.len(), 3);
    assert_eq!(resp.session_id, "sess-r");
}

#[test]
fn read_rejects_outside_sandbox() {
    let _tmp = setup_sandbox();
    let resp = raw_read(RawReadRequest {
        session_id: None,
        file: Some("/etc/passwd".to_string()),
        start_line: nz(1),
        end_line: nz(2),
    });
    assert!(resp.is_err());
}

#[test]
fn read_range_too_large() {
    let _tmp = setup_sandbox();
    let err = raw_read(RawReadRequest {
        session_id: Some("nope".to_string()),
        file: None,
        start_line: nz(1),
        end_line: nz(10_000),
    })
    .expect_err("should fail");
    assert!(err.to_string().contains("range_too_large"));
}
