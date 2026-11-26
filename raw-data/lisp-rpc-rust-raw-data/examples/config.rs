//! Example: Using Lisp-RPC Plain Mode (`lisp-rpc-rust-raw-data`) as a configuration file format.
//!
//! Run with:
//! `cargo run -p lisp-rpc-rust-raw-data --example config`

use lisp_rpc_rust_raw_data::{Data, files::DataFile};

fn main() -> anyhow::Result<()> {
    // 1. Single-root multi-line configuration with `;;` comments, nested named blocks,
    //    quoted anonymous maps `'(:k v)`, quoted lists `'(...)`, and optional defaults.
    let single_config = r#"
;; Service configuration (config.lisp)
(service-config
  :name "auth-service"
  :listen "0.0.0.0"
  :port 8080          ; HTTP port
  :tls 'false         ; explicit false symbol
  :log-level 'info
  :allowed-origins '("https://app.example.com" "http://localhost:3000")
  :labels '(:env "production" :region "us-east-1")
  :database (db-config
              :host "db.internal"
              :port 5432
              :pool-size 16))
"#;

    let cfg: Data<'static> = single_config.parse()?;
    assert_eq!(cfg.name(), Some("service-config"));

    // Remember: `.get(...)` and `get_*` expect keyword names WITHOUT a leading `:`
    let host = cfg.get_str("listen").unwrap_or("127.0.0.1");
    let port = cfg.get_u64("port").unwrap_or(3000);
    let tls = cfg.get_bool("tls").unwrap_or(false);
    let workers = cfg.get_u64("workers").unwrap_or(4); // optional default when key is omitted
    let log_level = cfg.get_symbol("log-level").unwrap_or("warn");

    let origins: Vec<&str> = cfg
        .get_slice("allowed-origins")
        .map(|items| items.iter().filter_map(Data::as_str).collect())
        .unwrap_or_default();

    let env = cfg
        .get_map("labels")
        .and_then(|m| m.get_str("env"))
        .unwrap_or("dev");

    let db = cfg.get_expr("database").expect("missing :database block");
    let db_host = db.get_str("host").unwrap_or("localhost");
    let db_pool = db.get_u64("pool-size").unwrap_or(8);

    println!(
        "service={:?} addr={host}:{port} tls={tls} workers={workers} log={log_level} env={env} origins={origins:?} db={db_host} (pool={db_pool})",
        cfg.get_str("name").unwrap()
    );

    // 2. Multi-section configuration file parsed via `DataFile`
    let multi_section_config = r#"
;; Multi-block application configuration
(http
  :host "0.0.0.0"
  :port 8080)

(metrics
  :enabled t
  :endpoint "/metrics")
"#;

    let data_file = DataFile::from_str_content(multi_section_config)?;
    let sections = data_file.gen_table();
    let http_port = sections
        .get("http")
        .and_then(|s| s.get_u64("port"))
        .unwrap_or(8080);
    let metrics_enabled = sections
        .get("metrics")
        .and_then(|s| s.get_bool("enabled"))
        .unwrap_or(false);

    println!("multi-section: http.port={http_port}, metrics.enabled={metrics_enabled}");
    Ok(())
}
