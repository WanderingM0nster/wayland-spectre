// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, time::Duration};
use futures_util::StreamExt;
use tokio::time::timeout;
use zbus::{zvariant::{OwnedObjectPath, OwnedValue, Value}, Connection, MessageStream, Proxy};
use crate::domain::types::{CaptureTestResult, Confidence, DiagnosticResult, Layer};

pub async fn check_pipewire() -> Vec<DiagnosticResult> {
    vec![check_pipewire_socket(), probe_portal_create_session().await]
}

pub async fn run_capture_test() -> Result<CaptureTestResult, String> {
    portal_capture_test().await
}

fn check_pipewire_socket() -> DiagnosticResult {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
    let socket = std::path::Path::new(&runtime_dir).join("pipewire-0");
    if socket.exists() {
        DiagnosticResult::pass(Layer::L4, "pipewire_socket",
            format!("PipeWire socket present: {}", socket.display()))
    } else {
        DiagnosticResult::fail(Layer::L4, "pipewire_socket",
            format!("PipeWire socket not found at {}", socket.display()),
            "systemctl --user start pipewire", Confidence::High)
    }
}

async fn probe_portal_create_session() -> DiagnosticResult {
    match portal_capture_test().await {
        Ok(r) if r.success => DiagnosticResult::pass(Layer::L4, "portal_create_session",
            r.format.as_deref().unwrap_or("Portal CreateSession succeeded")),
        Ok(r) => DiagnosticResult::fail(Layer::L4, "portal_create_session",
            r.error.unwrap_or_else(|| "Portal CreateSession failed".into()),
            "systemctl --user restart xdg-desktop-portal", Confidence::High),
        Err(e) => DiagnosticResult::fail(Layer::L4, "portal_create_session", e,
            "systemctl --user restart xdg-desktop-portal", Confidence::High),
    }
}

async fn portal_capture_test() -> Result<CaptureTestResult, String> {
    let conn = Connection::session().await
        .map_err(|e| format!("Cannot connect to session D-Bus: {e}"))?;
    let sender = conn.unique_name()
        .map(|n| n.as_str().trim_start_matches(':').replace('.', "_"))
        .unwrap_or_else(|| "0_0".into());
    let handle_token  = format!("wspectre_h{}", std::process::id());
    let session_token = format!("wspectre_s{}", std::process::id());
    let request_path  = format!("/org/freedesktop/portal/desktop/request/{}/{}", sender, handle_token);
    let mut stream = MessageStream::from(&conn);
    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("handle_token".into(),         Value::from(handle_token.as_str()));
    options.insert("session_handle_token".into(), Value::from(session_token.as_str()));
    let portal = Proxy::new(&conn, "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop", "org.freedesktop.portal.ScreenCast")
        .await.map_err(|e| format!("Cannot connect to ScreenCast interface: {e}"))?;
    let call_result = timeout(
        Duration::from_secs(8),
        portal.call::<_, _, (OwnedObjectPath,)>("CreateSession", &(options,)),
    ).await;
    match call_result {
        Err(_) => Ok(CaptureTestResult { success: false, node_id: None, width: None, height: None,
            format: None, error: Some("CreateSession timed out (Bug C / ELOOP). Check: journalctl --user -u xdg-desktop-portal -n 50".into()) }),
        Ok(Err(e)) => {
            let msg = e.to_string();
            let is_eloop = msg.contains("ELOOP") || msg.contains("Too many levels") || msg.contains("symbolic links");
            Ok(CaptureTestResult { success: false, node_id: None, width: None, height: None, format: None,
                error: Some(if is_eloop { format!("ELOOP confirms Bug C: {msg}") } else { format!("CreateSession error: {msg}") }) })
        }
        Ok(Ok((_handle,))) => {
            let rp = request_path.clone();
            let response = timeout(Duration::from_secs(5), async move {
                while let Some(Ok(msg)) = stream.next().await {
                    let h = msg.header();
                    if h.message_type() == zbus::message::Type::Signal
                        && h.path().map(|p| p.as_str() == rp.as_str()).unwrap_or(false)
                        && h.member().map(|m| m.as_str() == "Response").unwrap_or(false)
                    { return Some(msg); }
                }
                None
            }).await;
            match response {
                Err(_) => Ok(CaptureTestResult { success: false, node_id: None, width: None,
                    height: None, format: None, error: Some("Response signal timed out".into()) }),
                Ok(None) => Ok(CaptureTestResult { success: false, node_id: None, width: None,
                    height: None, format: None, error: Some("Stream closed before Response".into()) }),
                Ok(Some(msg)) => {
                    match msg.body().deserialize::<(u32, HashMap<String, OwnedValue>)>() {
                        Err(e) => Ok(CaptureTestResult { success: false, node_id: None,
                            width: None, height: None, format: None, error: Some(format!("Decode error: {e}")) }),
                        Ok((0, results)) => {
                            let sp = results.get("session_handle").map(|v| v.to_string()).unwrap_or_else(|| "<unknown>".into());
                            let _ = close_portal_session(&conn, &sp).await;
                            Ok(CaptureTestResult { success: true, node_id: None, width: None, height: None,
                                format: Some(format!("Session {sp} created and closed — Bug C not present")), error: None })
                        }
                        Ok((code, dict)) => {
                            let be = dict.get("error").map(|v| v.to_string()).unwrap_or_default();
                            let is_eloop = be.contains("ELOOP") || be.contains("Too many levels");
                            Ok(CaptureTestResult { success: false, node_id: None, width: None, height: None, format: None,
                                error: Some(if is_eloop { format!("Response {code} ELOOP Bug C: {be}") } else { format!("Response {code}: {be}") }) })
                        }
                    }
                }
            }
        }
    }
}

async fn close_portal_session(conn: &Connection, path: &str) -> zbus::Result<()> {
    let s = Proxy::new(conn, "org.freedesktop.portal.Desktop", path, "org.freedesktop.portal.Session").await?;
    let _: () = s.call("Close", &()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn socket_path() {
        assert_eq!(std::path::Path::new("/run/user/1000").join("pipewire-0").to_str().unwrap(), "/run/user/1000/pipewire-0");
    }
    #[test]
    fn eloop_detected() {
        let s = "ELOOP: Too many levels of symbolic links";
        assert!(s.contains("ELOOP") || s.contains("Too many levels") || s.contains("symbolic links"));
    }
    #[test]
    fn sender_normalisation() {
        assert_eq!(":1.47".trim_start_matches(':').replace('.', "_"), "1_47");
    }
}
