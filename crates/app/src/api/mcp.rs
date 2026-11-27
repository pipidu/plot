//! `sciplot --mcp`：stdio 上的 JSON-RPC。窗口开着就转给本机 API，否则在本进程里画。
//! stdout 只写协议消息，日志写到 stderr。

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};

use super::http::{self, Endpoint};
use super::tools::{self, ToolOutput};
use crate::prefs::Prefs;

enum Frame {
    Line,
    Length,
}

struct Session {
    project: sciplot_core::Project,
    path: Option<PathBuf>,
    font: Option<sciplot_plot::GlyphFont>,
    faces: Vec<Vec<u8>>,
    prefs: Prefs,
    forward: bool,
    told_forward: bool,
}

pub fn serve(project: Option<&str>) -> Result<(), String> {
    sciplot_script::install();
    let mut session = Session::open(project, true)?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    loop {
        match read_message(&mut reader)? {
            None => break,
            Some((frame, message)) => {
                if let Some(response) = session.reply(&message) {
                    write_message(&mut writer, &frame, &response)?;
                }
            }
        }
    }
    Ok(())
}

impl Session {
    fn open(project: Option<&str>, forward: bool) -> Result<Self, String> {
        let prefs = crate::prefs::load();
        sciplot_core::set_language(prefs.language);
        let loaded = crate::fonts::load(prefs.font_request());
        let font = crate::fonts::plot_glyphs(&loaded);
        let mut faces = vec![loaded.latin];
        if loaded.cjk != faces[0] {
            faces.push(loaded.cjk);
        }
        let (project, path) = match project {
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
                (sciplot_core::Project::from_json(&text)?, Some(PathBuf::from(path)))
            }
            None => (sciplot_core::Project::new(sciplot_core::t("未命名项目")), None),
        };
        Ok(Self {
            project,
            path,
            font,
            faces,
            prefs,
            forward,
            told_forward: false,
        })
    }

    fn reply(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        if method.is_empty() {
            return id.map(|id| rpc_error(id, -32600, "invalid request"));
        }
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or("2024-11-05"),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "sciplot", "version": env!("CARGO_PKG_VERSION")},
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_defs()})),
            "tools/call" => self.call_tool(&params),
            "notifications/initialized" | "notifications/cancelled" => return None,
            other if other.starts_with("notifications/") => return None,
            other => Err((-32601, format!("method not found: {other}"))),
        };
        let id = id?;
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => rpc_error(id, code, &message),
        })
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i32, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "tools/call needs a name".to_string()))?;
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        match self.execute(name, &arguments) {
            Ok(output) => Ok(mcp_result(&output, false)),
            Err(err) => Ok(mcp_result(
                &ToolOutput {
                    text: err,
                    ..ToolOutput::default()
                },
                true,
            )),
        }
    }

    fn execute(&mut self, name: &str, arguments: &Value) -> Result<ToolOutput, String> {
        if self.forward {
            if let Some(endpoint) = live_window() {
                if !self.told_forward {
                    if self.path.is_some() {
                        eprintln!(
                            "{}",
                            sciplot_core::tr!(
                                "窗口正在运行，调用交给窗口。--project 在这种情况下不使用。",
                                "A window is running, so calls go there. --project is not used in that case."
                            )
                        );
                    } else {
                        eprintln!(
                            "{}",
                            sciplot_core::tr!(
                                "窗口正在运行，调用交给窗口。",
                                "A window is running, so calls go there."
                            )
                        );
                    }
                    self.told_forward = true;
                }
                return forward(&endpoint, name, arguments);
            }
        }
        let faces = self.faces.clone();
        let refs: Vec<&[u8]> = faces.iter().map(Vec::as_slice).collect();
        let output = tools::dispatch(&mut self.project, self.font.as_ref(), &refs, &mut self.prefs, name, arguments)?;
        if !tools::readonly(name) {
            if output.replaced_project {
                self.path = output.project_path.as_ref().map(PathBuf::from);
            }
            if let Some(path) = &self.path {
                let text = self.project.to_json()?;
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                    }
                }
                std::fs::write(path, text).map_err(|err| err.to_string())?;
            }
        }
        Ok(output)
    }
}

fn tool_defs() -> Vec<Value> {
    tools::list()
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema,
            })
        })
        .collect()
}

fn mcp_result(output: &ToolOutput, is_error: bool) -> Value {
    let mut content = vec![json!({"type": "text", "text": output.text})];
    if let Some(png) = &output.png {
        content.push(json!({
            "type": "image",
            "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png),
            "mimeType": "image/png",
        }));
    }
    if let Some(svg) = &output.svg {
        content.push(json!({"type": "text", "text": svg}));
    }
    if let Some(file) = &output.file {
        content.push(json!({
            "type": "text",
            "text": format!(
                "{} {}",
                output.mime.as_deref().unwrap_or("application/octet-stream"),
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, file)
            ),
        }));
    }
    json!({"content": content, "isError": is_error})
}

fn live_window() -> Option<Endpoint> {
    let endpoint = http::read_endpoint(&crate::prefs::home_dir())?;
    if endpoint.pid == std::process::id() {
        return None;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(500))
        .timeout_connect(Duration::from_millis(200))
        .build();
    let url = format!("http://127.0.0.1:{}/api/health", endpoint.port);
    let ok = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {}", endpoint.token))
        .call()
        .map(|response| response.status() == 200)
        .unwrap_or(false);
    ok.then_some(endpoint)
}

fn forward(endpoint: &Endpoint, name: &str, arguments: &Value) -> Result<ToolOutput, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build();
    let url = format!("http://127.0.0.1:{}/api/call", endpoint.port);
    let response = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {}", endpoint.token))
        .set("Content-Type", "application/json")
        .send_string(&json!({"name": name, "arguments": arguments}).to_string())
        .map_err(|err| err.to_string())?;
    let body = response.into_string().map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    http::output_from_http(&value)
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<(Frame, Value)>, String> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|err| err.to_string())?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(length) = content_length(trimmed) {
            loop {
                line.clear();
                if reader.read_line(&mut line).map_err(|err| err.to_string())? == 0 {
                    return Err(sciplot_core::tr!("协议消息不完整", "The protocol message was incomplete").into());
                }
                if line.trim().is_empty() {
                    break;
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).map_err(|err| err.to_string())?;
            let value = serde_json::from_slice(&body).map_err(|err| err.to_string())?;
            return Ok(Some((Frame::Length, value)));
        }
        if trimmed.starts_with('{') {
            let value = serde_json::from_str(trimmed).map_err(|err| err.to_string())?;
            return Ok(Some((Frame::Line, value)));
        }
        eprintln!(
            "{}",
            sciplot_core::trf!("忽略无法识别的输入：{text}", "Ignoring unrecognized input: {text}", text = trimmed)
        );
    }
}

fn content_length(line: &str) -> Option<usize> {
    let rest = line
        .strip_prefix("Content-Length:")
        .or_else(|| line.strip_prefix("content-length:"))?;
    rest.trim().parse().ok()
}

fn write_message(writer: &mut impl Write, frame: &Frame, message: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(message).map_err(|err| err.to_string())?;
    match frame {
        Frame::Line => {
            writer.write_all(&body).map_err(|err| err.to_string())?;
            writer.write_all(b"\n").map_err(|err| err.to_string())?;
        }
        Frame::Length => {
            write!(writer, "Content-Length: {}\r\n\r\n", body.len()).map_err(|err| err.to_string())?;
            writer.write_all(&body).map_err(|err| err.to_string())?;
        }
    }
    writer.flush().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialize_list_and_call_stay_on_stdout_shape() {
        let mut session = Session::open(None, false).unwrap();
        let init = session
            .reply(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}))
            .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "sciplot");
        assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
        assert!(session.reply(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let listed = session.reply(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let names: Vec<_> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert!(names.contains(&"create_table"));
        assert!(names.contains(&"render_graph"));
        let called = session
            .reply(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "create_table",
                    "arguments": {
                        "name": "t",
                        "columns": [
                            {"name": "x", "values": [1, 2, 3]},
                            {"name": "y", "values": [1, 4, 9]}
                        ]
                    }
                }
            }))
            .unwrap();
        assert_eq!(called["result"]["isError"], false);
        assert_eq!(session.project.tables.len(), 1);
        let rendered = session
            .reply(&json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "render_graph",
                    "arguments": {
                        "graph": session.project.graphs.first().map(|graph| graph.id.raw()).unwrap_or(0),
                        "format": "svg",
                        "width": 320,
                        "height": 200
                    }
                }
            }))
            .unwrap();
        // 还没画图，渲染应报错，但协议层仍然是 result。
        assert!(rendered["result"]["isError"].as_bool().unwrap_or(false) || rendered.get("error").is_some());
        let unknown = session.reply(&json!({"jsonrpc":"2.0","id":5,"method":"nope"})).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
