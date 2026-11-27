//! 用子进程跑 `sciplot --mcp`，确认协议走 stdin/stdout。

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

#[test]
fn mcp_child_creates_a_table() {
    let home = std::env::temp_dir().join(format!(
        "sciplot-mcp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sciplot"))
        .arg("--mcp")
        .env("SCIPLOT_HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sciplot --mcp");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if tx.send(line.clone()).is_err() {
                break;
            }
            line.clear();
        }
    });

    let responses = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({
            "jsonrpc":"2.0",
            "id":3,
            "method":"tools/call",
            "params":{
                "name":"create_table",
                "arguments":{
                    "name":"测量",
                    "columns":[
                        {"name":"x","values":[1,2,3,4,5]},
                        {"name":"y","values":[1,4,9,16,25]}
                    ]
                }
            }
        }),
        json!({
            "jsonrpc":"2.0",
            "id":4,
            "method":"tools/call",
            "params":{
                "name":"plot_columns",
                "arguments":{"table":"测量","x":"x","y":["y"],"kind":"line"}
            }
        }),
        json!({
            "jsonrpc":"2.0",
            "id":5,
            "method":"tools/call",
            "params":{
                "name":"render_graph",
                "arguments":{"graph":"y-折线","format":"png","width":400,"height":280}
            }
        }),
    ];
    for message in &responses {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    let mut by_id = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while by_id.len() < 5 && std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(line) => {
                let value: Value = serde_json::from_str(line.trim()).unwrap_or_else(|err| panic!("{err}: {line}"));
                if value.get("id").is_some() {
                    by_id.push(value);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&home);

    assert_eq!(by_id.len(), 5, "只收到 {by_id:?}");
    assert_eq!(by_id[0]["result"]["serverInfo"]["name"], "sciplot");
    assert!(by_id[1]["result"]["tools"].as_array().unwrap().len() >= 8);
    assert_eq!(by_id[2]["result"]["isError"], false);
    assert_eq!(by_id[3]["result"]["isError"], false);
    let content = by_id[4]["result"]["content"].as_array().unwrap();
    assert!(content.iter().any(|item| item["type"] == "image" && item["mimeType"] == "image/png"));
    assert_eq!(by_id[4]["result"]["isError"], false);
}
