//! Example: Deserializing a multi-line `.lisp` config file into typed Rust structs via Serde.
//!
//! Run with:
//! `cargo run -p lisp-rpc-rust-serializer --example config`

use lisp_rpc_rust_serializer::lisp_rpc_from_str;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    #[serde(default = "default_pool_size")]
    pool_size: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct AppConfig {
    service_name: String,
    listen_addr: String,
    enable_tls: bool,
    cert_path: Option<String>,
    #[serde(default = "default_workers")]
    workers: usize,
    allowed_origins: Vec<String>,
    database: DatabaseConfig,
}

fn default_pool_size() -> u32 {
    8
}

fn default_workers() -> usize {
    4
}

/// Strips `;` / `;;` Lisp line comments outside of double-quoted strings.
fn strip_lisp_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let mut in_string = false;
        let mut escape = false;
        let mut cut = line.len();
        for (idx, ch) in line.char_indices() {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' if in_string => escape = true,
                '"' => in_string = !in_string,
                ';' if !in_string => {
                    cut = idx;
                    break;
                }
                _ => {}
            }
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

fn main() {
    let raw_config = r#"
;; Application configuration file (app.lisp)
;; Note: Serde's `lisp_rpc_from_str` automatically maps `:kebab-case` keys
;; to `snake_case` fields without `#[serde(rename_all = "kebab-case")]`.
(app-config
  :service-name "billing-api"
  :listen-addr "0.0.0.0:8080" ; HTTP bind address
  :enable-tls nil             ; `t` for true, `nil` for false
  :cert-path nil              ; `Option<String>::None`
  ;; `:workers` is omitted so `#[serde(default = "default_workers")]` applies
  :allowed-origins '("https://app.example.com" "http://localhost:3000")
  :database (database-config
              :host "db.internal"
              :port 5432))
"#;

    let clean = strip_lisp_comments(raw_config);
    let config: AppConfig = lisp_rpc_from_str(&clean).expect("failed to parse config");

    println!("Loaded typed config: {config:#?}");
    assert_eq!(config.service_name, "billing-api");
    assert!(!config.enable_tls);
    assert_eq!(config.workers, 4);
    assert_eq!(config.cert_path, None);
    assert_eq!(config.database.pool_size, 8);
}
