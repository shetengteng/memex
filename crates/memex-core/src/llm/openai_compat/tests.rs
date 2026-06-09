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
