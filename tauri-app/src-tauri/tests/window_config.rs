//! Window config regression test.
//!
//! 这次重构改了 tauri.conf.json 的 windows 数组（main 改大、新增 tray-popup）。
//! 直接读 conf 文件做断言，比起跑 webview 单元测试便宜得多——只要 JSON 结构
//! 错了，这里就能在 CI 上抓到。

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn load_conf() -> Value {
    let path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let conf_path = path.join("tauri.conf.json");
    let s = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", conf_path.display()));
    serde_json::from_str(&s).expect("tauri.conf.json must parse as JSON")
}

fn find_window<'a>(conf: &'a Value, label: &str) -> Option<&'a Value> {
    conf["app"]["windows"]
        .as_array()
        .expect("app.windows is array")
        .iter()
        .find(|w| w["label"].as_str() == Some(label))
}

#[test]
fn main_window_is_desktop_sized() {
    let conf = load_conf();
    let w = find_window(&conf, "main").expect("must have main window");
    assert_eq!(w["width"].as_f64(), Some(1100.0), "main width");
    assert_eq!(w["height"].as_f64(), Some(720.0), "main height");
    // 是真桌面窗口，不再透明 / 无边框
    assert_eq!(
        w["decorations"].as_bool(),
        Some(true),
        "main must have decorations (titlebar)"
    );
    assert_eq!(
        w["transparent"].as_bool(),
        Some(false),
        "main must not be transparent"
    );
    // 启动时不可见，等 Rust 端 setup() 调 main.show() 再亮（避免闪屏）
    assert_eq!(w["visible"].as_bool(), Some(false));
    assert_eq!(w["resizable"].as_bool(), Some(true));
}

#[test]
fn tray_popup_window_exists_with_correct_shape() {
    let conf = load_conf();
    let w = find_window(&conf, "tray-popup").expect("must have tray-popup window");

    // 360x520 是设计稿尺寸；改尺寸需要同步改 tray.rs 里的 reposition_to_tray_anchor
    assert_eq!(w["width"].as_f64(), Some(360.0));
    assert_eq!(w["height"].as_f64(), Some(520.0));

    // tray popup 必须是透明 + 无边框 + 不可调整大小
    assert_eq!(w["decorations"].as_bool(), Some(false));
    assert_eq!(w["transparent"].as_bool(), Some(true));
    assert_eq!(w["resizable"].as_bool(), Some(false));

    // 启动隐藏，跳过任务栏，置顶
    assert_eq!(w["visible"].as_bool(), Some(false));
    assert_eq!(w["skipTaskbar"].as_bool(), Some(true));
    assert_eq!(w["alwaysOnTop"].as_bool(), Some(true));

    // url 必须指向 #/tray-popup（不能少 #，少了走的就是浏览器 history 模式）
    let url = w["url"].as_str().expect("tray-popup url must be string");
    assert!(
        url.contains("#/tray-popup"),
        "tray-popup url should contain '#/tray-popup', got {url:?}"
    );
}

#[test]
fn capabilities_lists_both_windows() {
    // capabilities/default.json 的 windows 数组决定了哪些窗口能用 plugin API
    let path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let cap_path = path.join("capabilities/default.json");
    let s = fs::read_to_string(&cap_path).expect("read capabilities/default.json");
    let v: Value = serde_json::from_str(&s).expect("parse capabilities/default.json");

    let windows = v["windows"].as_array().expect("capabilities.windows is array");
    let labels: Vec<&str> = windows.iter().filter_map(|x| x.as_str()).collect();
    assert!(labels.contains(&"main"), "capabilities must allow 'main'");
    assert!(
        labels.contains(&"tray-popup"),
        "capabilities must allow 'tray-popup'"
    );
    // 应该没有遗留的 dashboard
    assert!(
        !labels.contains(&"dashboard"),
        "stale 'dashboard' window label still in capabilities"
    );
}
