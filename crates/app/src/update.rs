//! 向 GitHub 查询最新发行版，和当前版本比较。

use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use crate::app::{Dialog, SciPlotApp};
use crate::i18n;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateView {
    Checking,
    Current,
    Newer { version: String, page: String, notes: String },
    Failed(String),
}

pub fn start(app: &mut SciPlotApp) {
    let (tx, rx) = mpsc::channel();
    app.update_rx = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(fetch_latest());
    });
    app.dialog = Some(Dialog::Update(UpdateView::Checking));
}

pub fn poll(app: &mut SciPlotApp) {
    let message = app.update_rx.as_ref().map(|rx| rx.try_recv());
    match message {
        Some(Ok(view)) => {
            app.update_rx = None;
            if matches!(app.dialog, Some(Dialog::Update(_))) {
                app.dialog = Some(Dialog::Update(view));
            }
        }
        Some(Err(TryRecvError::Disconnected)) => app.update_rx = None,
        _ => {}
    }
}

fn fetch_latest() -> UpdateView {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).build();
    let response = agent
        .get(i18n::RELEASES_API)
        .set("User-Agent", &format!("SciPlot/{}", i18n::VERSION))
        .set("Accept", "application/vnd.github+json")
        .call();
    match response {
        Ok(response) => match response.into_string() {
            Ok(body) => interpret(i18n::VERSION, &body),
            Err(_) => UpdateView::Failed("GitHub 的回复读不完整。".into()),
        },
        Err(ureq::Error::Status(code, response)) => {
            let detail = response.into_string().unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&detail)
                .ok()
                .and_then(|value| value.get("message").and_then(|item| item.as_str()).map(str::to_string));
            match message {
                Some(message) => UpdateView::Failed(format!("检查更新失败：{message}")),
                None => UpdateView::Failed(format!("检查更新失败（HTTP {code}）。")),
            }
        }
        Err(_) => UpdateView::Failed("连不上 GitHub，请检查网络后再试。".into()),
    }
}

pub fn interpret(current: &str, body: &str) -> UpdateView {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return UpdateView::Failed("GitHub 返回的内容无法解析。".into()),
    };
    let Some(tag) = value.get("tag_name").and_then(|item| item.as_str()) else {
        let message = value.get("message").and_then(|item| item.as_str()).unwrap_or("GitHub 没有给出版本号。");
        return UpdateView::Failed(format!("检查更新失败：{message}"));
    };
    let page = value.get("html_url").and_then(|item| item.as_str()).unwrap_or(i18n::RELEASES_URL).to_string();
    let notes = value.get("body").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    if is_newer(tag, current) {
        UpdateView::Newer { version: display_version(tag), page, notes }
    } else {
        UpdateView::Current
    }
}

fn display_version(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let mut parts = text.split('.');
    let major = parts.next()?.split(['-', '+']).next()?.parse().ok()?;
    let minor = parts.next()?.split(['-', '+']).next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").split(['-', '+']).next()?.parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_release_is_detected_numerically() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.10.0", "0.9.1"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
    }

    #[test]
    fn release_json_becomes_a_dialog_state() {
        let body = r#"{"tag_name":"v0.9.0","html_url":"https://github.com/pipidu/plot/releases/tag/v0.9.0","body":"新版本说明"}"#;
        match interpret("0.2.0", body) {
            UpdateView::Newer { version, page, notes } => {
                assert_eq!(version, "0.9.0");
                assert!(page.contains("v0.9.0"));
                assert_eq!(notes, "新版本说明");
            }
            other => panic!("应当发现新版本，得到 {other:?}"),
        }
        assert_eq!(interpret("0.9.0", body), UpdateView::Current);
        assert!(matches!(interpret("0.2.0", "不是 json"), UpdateView::Failed(_)));
    }
}
