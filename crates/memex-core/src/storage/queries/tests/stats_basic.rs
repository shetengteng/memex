//! Stats / project-summary aggregation defenses. Workload-side tests
//! live in sibling files.

use super::ws_seed_session;
use crate::storage::db::Db;

/// 复现「资料库左侧很多展示的过滤条件值空」：如果数据库里存在
/// `project_path = ''` 的 session（早期写入路径漏卡空串），老 SQL 仅
/// `IS NOT NULL` 会让这些行聚合成一条 path/name 全空的 ProjectSummary，
/// 流到前端就是空 label 的 facet 行。
///
/// 修复后 SQL 同时排除 `project_path = ''`，只把合法 path 的项目返回。
#[test]
fn list_project_summaries_excludes_empty_path() {
    let db = Db::open_in_memory().unwrap();
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();

    ws_seed_session(&db, "s1", "cursor", Some("/Users/me/real"), &now, 10);
    ws_seed_session(&db, "s2", "cursor", Some(""), &now, 5);
    ws_seed_session(&db, "s3", "cursor", None, &now, 3);

    let summaries = db.list_project_summaries().unwrap();

    assert_eq!(summaries.len(), 1, "only the non-empty project_path row should survive");
    assert_eq!(summaries[0].project_path, "/Users/me/real");
    assert_eq!(summaries[0].session_count, 1);
}
