//! 窗口进程里的本机 HTTP API。只监听 127.0.0.1，请求要带启动时生成的令牌。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

use super::tools::{self, ToolOutput};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Endpoint {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

pub struct Job {
    pub name: String,
    pub arguments: Value,
    pub reply: Sender<Result<ToolOutput, String>>,
}

pub struct ApiServer {
    rx: Receiver<Job>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    endpoint_path: PathBuf,
    token: String,
    requested_port: u16,
}

impl ApiServer {
    pub fn requested_port(&self) -> u16 {
        self.requested_port
    }

    pub fn poll(&self) -> Vec<Job> {
        let mut jobs = Vec::new();
        while let Ok(job) = self.rx.try_recv() {
            jobs.push(job);
        }
        jobs
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        remove_if_ours(&self.endpoint_path, &self.token);
    }
}

pub fn start(requested: u16, home: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> Result<ApiServer, String> {
    let token = random_token()?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
    let mut last_error = String::new();
    let mut bound = None;
    for port in port_candidates(requested) {
        match Server::http(format!("127.0.0.1:{port}")) {
            Ok(server) => {
                bound = Some(server);
                break;
            }
            Err(err) => last_error = err.to_string(),
        }
    }
    let server = bound.ok_or_else(|| {
        sciplot_core::trf!(
            "本机 API 没能启动：{err}",
            "The local API failed to start: {err}",
            err = last_error
        )
    })?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.port())
        .unwrap_or(requested);
    let endpoint_path = endpoint_file(&home);
    write_endpoint(&endpoint_path, port, &token)?;
    let shutdown_thread = Arc::clone(&shutdown);
    let thread_token = token.clone();
    let thread = thread::spawn(move || serve(server, thread_token, tx, wake, shutdown_thread));
    Ok(ApiServer {
        rx,
        shutdown,
        thread: Some(thread),
        endpoint_path,
        token,
        requested_port: requested,
    })
}

pub(crate) fn endpoint_file(home: &Path) -> PathBuf {
    home.join(".sciplot_api.json")
}

pub(crate) fn read_endpoint(home: &Path) -> Option<Endpoint> {
    let text = std::fs::read_to_string(endpoint_file(home)).ok()?;
    serde_json::from_str(&text).ok()
}

fn port_candidates(requested: u16) -> Vec<u16> {
    if requested == 0 {
        return vec![0];
    }
    let mut ports = vec![requested];
    for port in 8765..8776 {
        if port != requested {
            ports.push(port);
        }
    }
    ports
}

fn serve(
    server: Server,
    token: String,
    tx: Sender<Job>,
    wake: Arc<dyn Fn() + Send + Sync>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        let request = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            Err(_) => break,
        };
        handle_request(request, &token, &tx, &wake, &shutdown);
    }
}

fn handle_request(
    mut request: Request,
    token: &str,
    tx: &Sender<Job>,
    wake: &Arc<dyn Fn() + Send + Sync>,
    shutdown: &AtomicBool,
) {
    if !authorized(&request, token) {
        let _ = respond(request, 401, json!({"ok": false, "error": "unauthorized"}));
        return;
    }
    let method = request.method().clone();
    let path = request.url().split('?').next().unwrap_or("").to_string();
    match (method, path.as_str()) {
        (Method::Get, "/api/health") => {
            let _ = respond(request, 200, json!({"ok": true, "name": "sciplot"}));
        }
        (Method::Get, "/api/tools") => {
            let tools: Vec<_> = tools::list()
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.input_schema,
                    })
                })
                .collect();
            let _ = respond(request, 200, json!({"ok": true, "tools": tools}));
        }
        (Method::Post, "/api/call") => {
            let body = match read_limited(request.as_reader(), 8 * 1024 * 1024) {
                Ok(body) => body,
                Err(err) => {
                    let _ = respond(request, 400, json!({"ok": false, "error": err}));
                    return;
                }
            };
            let parsed: Value = match serde_json::from_str(&body) {
                Ok(value) => value,
                Err(err) => {
                    let _ = respond(request, 400, json!({"ok": false, "error": err.to_string()}));
                    return;
                }
            };
            let name = parsed.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                let _ = respond(
                    request,
                    400,
                    json!({"ok": false, "error": sciplot_core::tr!("缺少参数 name", "Missing argument name")}),
                );
                return;
            }
            let arguments = parsed.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let (reply_tx, reply_rx) = mpsc::channel();
            if tx.send(Job { name, arguments, reply: reply_tx }).is_err() {
                let _ = respond(request, 503, json!({"ok": false, "error": "window closed"}));
                return;
            }
            wake();
            let result = wait_reply(reply_rx, shutdown);
            let status = if result.is_ok() { 200 } else { 500 };
            let _ = respond(request, status, http_body(result));
        }
        _ => {
            let _ = respond(request, 404, json!({"ok": false, "error": "not found"}));
        }
    }
}

fn read_limited(reader: &mut dyn Read, limit: usize) -> Result<String, String> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 8192];
    while body.len() < limit {
        let max = chunk.len().min(limit - body.len());
        let read = reader.read(&mut chunk[..max]).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8(body).map_err(|_| sciplot_core::tr!("请求体不是 UTF-8", "The request body is not UTF-8").to_string())
}

fn wait_reply(reply_rx: Receiver<Result<ToolOutput, String>>, shutdown: &AtomicBool) -> Result<ToolOutput, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Err(sciplot_core::tr!("窗口正在关闭", "The window is closing").into());
        }
        match reply_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) if std::time::Instant::now() < deadline => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(sciplot_core::tr!("窗口没有在 60 秒内回答", "The window did not answer within 60 seconds").into());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(sciplot_core::tr!("窗口已关闭", "The window has closed").into());
            }
        }
    }
}

fn http_body(result: Result<ToolOutput, String>) -> Value {
    match result {
        Ok(output) => json!({
            "ok": true,
            "text": output.text,
            "data": output.data,
            "png_base64": output.png.as_ref().map(|bytes| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)),
            "svg": output.svg,
            "graph_id": output.graph.map(|id| id.raw()),
            "table_id": output.table.map(|id| id.raw()),
            "project_path": output.project_path,
            "replaced_project": output.replaced_project,
        }),
        Err(err) => json!({"ok": false, "error": err}),
    }
}

pub(crate) fn output_from_http(value: &Value) -> Result<ToolOutput, String> {
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = value.get("error").and_then(Value::as_str).unwrap_or("api error");
        return Err(err.to_string());
    }
    let mut output = ToolOutput {
        text: value.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
        data: value.get("data").cloned().unwrap_or(Value::Null),
        png: None,
        svg: value.get("svg").and_then(Value::as_str).map(str::to_string),
        graph: value.get("graph_id").and_then(Value::as_u64).map(sciplot_core::GraphId),
        table: value.get("table_id").and_then(Value::as_u64).map(sciplot_core::TableId),
        open_graph: false,
        open_table: false,
        project_path: value.get("project_path").and_then(Value::as_str).map(str::to_string),
        replaced_project: value.get("replaced_project").and_then(Value::as_bool).unwrap_or(false),
    };
    if let Some(encoded) = value.get("png_base64").and_then(Value::as_str) {
        output.png = Some(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                .map_err(|err| err.to_string())?,
        );
    }
    Ok(output)
}

fn authorized(request: &Request, token: &str) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && header
                .value
                .as_str()
                .strip_prefix("Bearer ")
                .is_some_and(|given| tokens_match(given.trim(), token))
    })
}

fn tokens_match(given: &str, expected: &str) -> bool {
    let given = given.as_bytes();
    let expected = expected.as_bytes();
    if given.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in given.iter().zip(expected) {
        diff |= left ^ right;
    }
    diff == 0
}

fn respond(request: Request, status: u16, body: Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(&body).map_err(|err| err.to_string())?;
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).map_err(|_| "header".to_string())?;
    let response = Response::from_data(bytes).with_status_code(status).with_header(header);
    request.respond(response).map_err(|err| err.to_string())
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).map_err(|err| err.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn write_endpoint(path: &Path, port: u16, token: &str) -> Result<(), String> {
    let endpoint = Endpoint { port, token: token.to_string(), pid: std::process::id() };
    let text = serde_json::to_string_pretty(&endpoint).map_err(|err| err.to_string())?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|err| err.to_string())?;
    file.write_all(text.as_bytes()).map_err(|err| err.to_string())?;
    Ok(())
}

fn remove_if_ours(path: &Path, token: &str) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(endpoint) = serde_json::from_str::<Endpoint>(&text) else {
        return;
    };
    // 换端口时新服务已经写下自己的令牌。旧服务退出不能把新文件删掉。
    if endpoint.pid == std::process::id() && endpoint.token == token {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn health_requires_token_and_call_reaches_the_window_side() {
        let home = std::env::temp_dir().join(format!("sciplot-api-test-{}", unique_suffix()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let server = start(0, home.clone(), || {}).unwrap();
        let endpoint = read_endpoint(&home).expect("endpoint file");
        assert_ne!(endpoint.port, 0);
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(5)).build();
        let health = format!("http://127.0.0.1:{}/api/health", endpoint.port);
        assert!(agent.get(&health).call().is_err());
        let response = agent
            .get(&health)
            .set("Authorization", &format!("Bearer {}", endpoint.token))
            .call()
            .unwrap();
        assert_eq!(response.status(), 200);

        let call_url = format!("http://127.0.0.1:{}/api/call", endpoint.port);
        let token = endpoint.token.clone();
        let worker = thread::spawn(move || {
            agent
                .post(&call_url)
                .set("Authorization", &format!("Bearer {token}"))
                .set("Content-Type", "application/json")
                .send_string(&json!({"name": "get_project", "arguments": {}}).to_string())
                .unwrap()
                .into_string()
                .unwrap()
        });
        let started = std::time::Instant::now();
        let mut answered = false;
        while started.elapsed() < Duration::from_secs(5) {
            for job in server.poll() {
                assert_eq!(job.name, "get_project");
                let _ = job.reply.send(Ok(ToolOutput {
                    text: "ok".into(),
                    data: json!({"name": "p"}),
                    ..ToolOutput::default()
                }));
                answered = true;
            }
            if answered {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let body = worker.join().unwrap();
        let value: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["name"], "p");
        let replacement = start(0, home.clone(), || {}).unwrap();
        drop(server);
        let kept = read_endpoint(&home).expect("the replacement keeps the endpoint file");
        assert_ne!(kept.token, endpoint.token);
        drop(replacement);
        assert!(!endpoint_file(&home).exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    fn unique_suffix() -> String {
        format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())
    }
}
