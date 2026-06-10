use super::*;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn serve_json_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[test]
fn test_provider_name() {
    let p = OpenAiCompatProvider::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "key",
        "deepseek-chat",
    );
    assert_eq!(p.name(), "deepseek");
}

#[test]
fn test_is_available() {
    let p = OpenAiCompatProvider::new("ds", "https://api.deepseek.com/v1", "key", "model");
    assert!(p.is_available());
    let empty = OpenAiCompatProvider::new("ds", "https://api.deepseek.com/v1", "", "model");
    assert!(!empty.is_available());
}

#[test]
fn test_api_root_with_v1() {
    let p = OpenAiCompatProvider::new("ds", "https://api.deepseek.com/v1", "k", "m");
    assert_eq!(p.api_root(), "https://api.deepseek.com/v1");
}

#[test]
fn test_api_root_without_v1() {
    let p = OpenAiCompatProvider::new("ds", "http://localhost:8080", "k", "m");
    assert_eq!(p.api_root(), "http://localhost:8080/v1");
}

#[test]
fn list_models_sorts_and_deduplicates_ids() {
    let base_url = serve_json_once(r#"{"data":[{"id":"zeta"},{"id":"alpha"},{"id":"alpha"}]}"#);
    let p = OpenAiCompatProvider::new("mock", &base_url, "key", "model");

    let models = p.list_models().unwrap();

    assert_eq!(models, vec!["alpha".to_string(), "zeta".to_string()]);
}

#[test]
fn generate_parses_chat_completion_response() {
    let base_url = serve_json_once(
        r#"{"choices":[{"message":{"content":"hello"}},{"message":{"content":" world"}}],"model":"mock-model","usage":{"completion_tokens":7}}"#,
    );
    let p = OpenAiCompatProvider::new("mock", &base_url, "key", "fallback-model");

    let response = p
        .generate(
            &LlmRequest::with_prompt("Say hello")
                .with_system("system")
                .with_max_tokens(32),
        )
        .unwrap();

    assert_eq!(response.text, "hello world");
    assert_eq!(response.model, "mock-model");
    assert_eq!(response.tokens_used, 7);
}

#[test]
fn generate_rejects_empty_content() {
    let base_url = serve_json_once(r#"{"choices":[{"message":{"content":"   "}}],"model":"m"}"#);
    let p = OpenAiCompatProvider::new("mock", &base_url, "key", "fallback-model");

    let err = match p.generate(&LlmRequest::with_prompt("empty")) {
        Ok(_) => panic!("empty content response should fail"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("empty content"));
}

/// 回归：DeepSeek-R1 / V4 等 reasoning model 在 max_tokens 太小时把所有
/// token 都花在 `reasoning_content` 上，`content` 是空 —— 之前会被报成
/// 通用的 "empty content"，让用户摸不着头脑（"我已经设置了 API key
/// 怎么还是不行？"）。这条用例守住「检测到 reasoning_content + content 空 →
/// 报 "请增大 max_tokens"」的契约。
#[test]
fn generate_emits_actionable_hint_when_reasoning_model_runs_out_of_tokens() {
    let body = r#"{"choices":[{"message":{"content":"","reasoning_content":"Let me think...the answer should be OK but I've used up all my budget"},"finish_reason":"length"}],"model":"deepseek-reasoner"}"#;
    let base_url = serve_json_once(body);
    let p = OpenAiCompatProvider::new("DeepSeek", &base_url, "key", "deepseek-reasoner");

    let err = match p.generate(&LlmRequest::with_prompt("Say OK").with_max_tokens(8)) {
        Ok(_) => panic!("reasoning model with empty content should error out with hint"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(msg.contains("reasoning"), "msg should mention reasoning: {msg}");
    assert!(
        msg.contains("max_tokens"),
        "msg should suggest increasing max_tokens: {msg}"
    );
    assert!(msg.contains("DeepSeek"), "msg should name provider: {msg}");
}

/// 回归：finish_reason="length" 但没有 reasoning_content（普通模型的极端
/// 情况，例如 prompt 引导出长串空白后 token 耗尽）。报错应该提示截断和
/// max_tokens，而不是含糊的 "empty content"。
#[test]
fn generate_emits_truncation_hint_when_finish_reason_is_length() {
    let body =
        r#"{"choices":[{"message":{"content":"   "},"finish_reason":"length"}],"model":"some-llm"}"#;
    let base_url = serve_json_once(body);
    let p = OpenAiCompatProvider::new("MockProvider", &base_url, "key", "some-llm");

    let err = match p.generate(&LlmRequest::with_prompt("hi").with_max_tokens(4)) {
        Ok(_) => panic!("finish_reason=length should error out with truncation hint"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(msg.contains("length"), "msg should mention length: {msg}");
    assert!(
        msg.contains("max_tokens"),
        "msg should suggest increasing max_tokens: {msg}"
    );
}

/// 普通模型回应正常（content 非空，没有 reasoning_content）—— 走 happy path,
/// 不被新增的诊断逻辑误伤。
#[test]
fn generate_keeps_happy_path_for_models_without_reasoning_content() {
    let body =
        r#"{"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}],"model":"gpt-4"}"#;
    let base_url = serve_json_once(body);
    let p = OpenAiCompatProvider::new("OpenAI", &base_url, "key", "gpt-4");

    let resp = p
        .generate(&LlmRequest::with_prompt("Say OK").with_max_tokens(32))
        .expect("happy path must succeed");
    assert_eq!(resp.text, "OK");
    assert_eq!(resp.model, "gpt-4");
}
