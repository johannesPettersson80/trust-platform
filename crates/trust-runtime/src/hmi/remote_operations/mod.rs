//! Named owner sessions for cross-runtime HMI commands. Tokens stay on the serving runtime.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::access::sessions::SessionInfo;
use super::commands::write::{path_text, WriteAction, WriteRefused, Written};
use super::member_paths::MemberPath;
use super::remote_values::remote_path;
use crate::value::Value;
use serde_json::{json, Value as Json};

mod owner;
pub use owner::configure_owner;
type Transport = dyn Fn(&str, &Json) -> Result<Json, String> + Send + Sync;
type Owner = dyn Fn(&Json) -> Json + Send + Sync;

struct OwnerSession {
    token: String,
    expires: Instant,
}

/// A transport boundary, not an alternate command engine. No token is returned to the browser.
#[derive(Default)]
pub struct RemoteOperations {
    published: Mutex<Vec<String>>,
    transport: Mutex<Option<Arc<Transport>>>,
    owner: Mutex<Option<Arc<Owner>>>,
    sessions: Mutex<BTreeMap<(String, String), OwnerSession>>,
}
impl std::fmt::Debug for RemoteOperations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteOperations").finish_non_exhaustive()
    }
}
impl RemoteOperations {
    /// Update the explicitly published local paths; no container expansion is implied.
    pub fn publish(&self, paths: Vec<String>) {
        if let Ok(mut published) = self.published.lock() {
            *published = paths
                .into_iter()
                .map(|path| super::remote_values::qualified_key(&path))
                .collect();
        }
    }
    /// Whether the owner explicitly publishes this path.
    pub fn is_published(&self, path: &str) -> bool {
        self.published
            .lock()
            .map(|paths| paths.contains(&super::remote_values::qualified_key(path)))
            .unwrap_or(false)
    }
    /// Install only the authenticated, encrypted mesh transport owned by the host.
    pub fn transport(&self, transport: Arc<Transport>) {
        if let Ok(mut slot) = self.transport.lock() {
            *slot = Some(transport);
        }
    }
    /// Install the owner's existing session and policy handler.
    pub fn owner(&self, owner: Arc<Owner>) {
        if let Ok(mut slot) = self.owner.lock() {
            *slot = Some(owner);
        }
    }
    /// Handle a bounded authenticated mesh request.
    pub fn receive(&self, request: &Json) -> Json {
        let handler = self.owner.lock().ok().and_then(|slot| slot.clone());
        handler.map_or_else(
            || json!({"ok":false,"error":"the owning HMI is unavailable"}),
            |handler| handler(request),
        )
    }
    fn call(&self, peer: &str, request: &Json) -> Result<Json, String> {
        if peer.is_empty()
            || peer.len() > 128
            || !peer
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err("invalid owning runtime".into());
        }
        let transport = self
            .transport
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .ok_or("remote HMI operation requires authenticated TLS mesh")?;
        transport(peer, request)
    }
    /// Explicit owner login; requires an already-authenticated local session.
    pub fn login(
        &self,
        local: &SessionInfo,
        peer: &str,
        user: &str,
        password: &str,
    ) -> Result<Json, String> {
        let answer = self.call(
            peer,
            &json!({"action":"login","user":user,"password":password}),
        )?;
        let token = answer.get("token").and_then(Json::as_str).ok_or_else(|| {
            answer["error"]
                .as_str()
                .unwrap_or("owner login refused")
                .to_string()
        })?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "remote sessions unavailable")?;
        sessions.retain(|_, session| session.expires > Instant::now());
        if sessions.len() >= 256
            && !sessions.contains_key(&(local.id.to_string(), peer.to_string()))
        {
            return Err("remote session limit reached; log out an unused session".into());
        }
        sessions.insert(
            (local.id.to_string(), peer.to_string()),
            OwnerSession {
                token: token.to_string(),
                expires: Instant::now() + local.expires_in.min(local.idle_expires_in),
            },
        );
        Ok(json!({"ok":true,"user":answer["user"],"role":answer["role"],"runtime":peer}))
    }
    /// Forget delegated credentials when the originating session logs out.
    pub fn logout(&self, local: &SessionInfo) {
        let removed = if let Ok(mut sessions) = self.sessions.lock() {
            let keys: Vec<_> = sessions
                .keys()
                .filter(|(id, _)| id == local.id.as_str())
                .cloned()
                .collect();
            keys.into_iter()
                .filter_map(|key| sessions.remove(&key).map(|token| (key.1, token)))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let transport = self.transport.lock().ok().and_then(|slot| slot.clone());
        if let Some(transport) = transport {
            std::thread::spawn(move || {
                for (peer, token) in removed {
                    let _ = transport(&peer, &json!({"action":"logout","token":token.token}));
                }
            });
        }
    }
    fn owner_token(&self, local: &SessionInfo, peer: &str) -> Option<String> {
        let mut sessions = self.sessions.lock().ok()?;
        sessions.retain(|_, session| session.expires > Instant::now());
        let session = sessions.get_mut(&(local.id.to_string(), peer.to_string()))?;
        session.expires = Instant::now() + local.expires_in.min(local.idle_expires_in);
        Some(session.token.clone())
    }
    /// Observe a fresh owner Hmi.Display sample after a command, not the background cache.
    pub fn observe(&self, local: &SessionInfo, path: &MemberPath) -> Option<Json> {
        let (peer, target) = remote_path(path)?;
        let token = self.owner_token(local, &peer)?;
        let answer = self
            .call(
                &peer,
                &json!({"action":"read","token":token,"path":path_text(&target)}),
            )
            .ok()?;
        if answer["ok"] != true || answer["value"].get("staleMs").is_some() {
            return None;
        }
        answer["value"].get("value").cloned()
    }
    /// Send a native write to the actual owner. A missing/expired owner login is explicit.
    pub fn write(
        &self,
        local: &SessionInfo,
        path: &MemberPath,
        action: &WriteAction,
        id: &str,
        pulse: Option<Duration>,
    ) -> Option<Result<Written, WriteRefused>> {
        let (peer, target) = remote_path(path)?;
        let token = self.owner_token(local, &peer);
        let Some(token) = token else {
            return Some(Err(WriteRefused::owner_login(&peer)));
        };
        let action = match action {
            WriteAction::Set(value) => json!({"set":value}),
            WriteAction::Toggle => json!({"toggle":true}),
            WriteAction::Choice { name, index } => json!({"choice":{"name":name,"index":index}}),
        };
        let request = json!({"action":"write","token":token,"id":id,"path":path_text(&target),"write":action,"pulse_ms":pulse.map(|duration|duration.as_millis() as u64)});
        match self.call(&peer, &request) {
            Err(reason) if reason.starts_with("unknown:") => None,
            Err(reason) => Some(Err(reason.into())),
            Ok(answer) => {
                if answer["login_required"] == true {
                    if let Ok(mut sessions) = self.sessions.lock() {
                        sessions.remove(&(local.id.to_string(), peer.clone()));
                    }
                    return Some(Err(WriteRefused::owner_login(&peer)));
                }
                if answer["unknown"] == true {
                    return None;
                }
                if answer["ok"] != true {
                    return Some(Err(answer["error"]
                        .as_str()
                        .unwrap_or("the owner refused the command")
                        .into()));
                }
                Some(
                    match (scalar(&answer["old"]), scalar(&answer["readback"])) {
                        (Some(old), Some(readback)) => Ok(Written { old, readback }),
                        _ => Err(
                            "the owner returned an unsupported readback; inspect its state".into(),
                        ),
                    },
                )
            }
        }
    }
}
fn scalar(value: &Json) -> Option<Value> {
    match value {
        Json::Bool(value) => Some(Value::Bool(*value)),
        Json::Number(value) => value
            .as_i64()
            .map(Value::LInt)
            .or_else(|| value.as_u64().map(Value::ULInt))
            .or_else(|| value.as_f64().map(Value::LReal)),
        Json::String(value) => Some(Value::String(value.as_str().into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::AccessRole;
    fn local() -> SessionInfo {
        SessionInfo {
            id: "local".into(),
            user: "operator".into(),
            role: AccessRole::Operator,
            idle_expires_in: Duration::from_secs(60),
            expires_in: Duration::from_secs(3600),
        }
    }
    #[test]
    fn owner_login_is_explicit_tokens_stay_private_and_logout_removes_authority() {
        let hub = RemoteOperations::default();
        let path = MemberPath::parse("Owner:Main.Setpoint").unwrap();
        // Specification 39: the refusal names the runtime to log in to as its own field, and the
        // audit's reason keeps `owner-login:<runtime>`. Catches a display left to parse the reason.
        let refused = hub
            .write(&local(), &path, &WriteAction::Set(json!(5)), "one", None)
            .unwrap()
            .unwrap_err();
        assert_eq!(refused.login.as_deref(), Some("Owner"));
        assert_eq!(refused.reason, "owner-login:Owner");
        hub.transport(Arc::new(|_, request| {
            if request["action"] == "login" {
                Ok(json!({"token":"private-owner-token","user":"owner-user","role":"operator"}))
            } else {
                assert_eq!(request["token"], "private-owner-token");
                Ok(json!({"ok":true,"old":4,"readback":5}))
            }
        }));
        let answer = hub
            .login(&local(), "Owner", "owner-user", "secret")
            .unwrap();
        assert!(answer.get("token").is_none());
        let written = hub
            .write(&local(), &path, &WriteAction::Set(json!(5)), "one", None)
            .unwrap()
            .unwrap();
        assert_eq!(written.readback, Value::LInt(5));
        hub.logout(&local());
        assert!(hub
            .write(&local(), &path, &WriteAction::Set(json!(5)), "two", None)
            .unwrap()
            .is_err());
    }
    #[test]
    fn a_post_send_transport_timeout_is_unknown_not_a_definite_refusal() {
        let hub = RemoteOperations::default();
        hub.sessions.lock().unwrap().insert(
            ("local".into(), "Owner".into()),
            OwnerSession {
                token: "token".into(),
                expires: Instant::now() + Duration::from_secs(60),
            },
        );
        hub.transport(Arc::new(|_, _| Err("unknown: lost reply".into())));
        assert!(hub
            .write(
                &local(),
                &MemberPath::parse("Owner:Main.Level").unwrap(),
                &WriteAction::Set(json!(4)),
                "id",
                None
            )
            .is_none());
    }
}
