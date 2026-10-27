//! Owning-runtime authorization, idempotency, native writes and audit for remote HMI commands.
use crate::control::ControlAuditEvent;
use crate::hmi::access::config::HmiAccessConfig;
use crate::hmi::access::policy::{authorize, rule_for, within_limits, Permission};
use crate::hmi::access::sessions::SessionStore;
use crate::hmi::commands::write::WriteAction;
use crate::hmi::exchange::HmiExchange;
use crate::hmi::member_paths::MemberPath;
use crate::hmi::write_value::json_of;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Bind the same named sessions and owner policy used by local browser operation.
pub fn configure_owner(
    exchange: &Arc<HmiExchange>,
    sessions: Arc<SessionStore>,
    access: Arc<HmiAccessConfig>,
    audit: Option<Sender<ControlAuditEvent>>,
) {
    let weak = Arc::downgrade(exchange);
    let receipts = Mutex::new(BTreeMap::<String, (Json, Json, Instant)>::new());
    exchange.remote_operations.owner(Arc::new(move |request| {
        if request["action"]=="login" {
            let (Some(user),Some(password))=(request["user"].as_str(),request["password"].as_str()) else { return json!({"ok":false,"error":"provide a named owner login"}); };
            return match sessions.login(user,password) {
                Ok((token,info))=>json!({"ok":true,"token":token,"user":info.user.as_str(),"role":info.role}),
                Err(_)=>json!({"ok":false,"error":"owner login refused or temporarily locked"}),
            };
        }
        let Some(info)=request["token"].as_str().and_then(|token|sessions.touch(token)) else { return json!({"ok":false,"login_required":true}); };
        if request["action"]=="logout" {
            if let Some(token)=request["token"].as_str(){sessions.logout(token);}
            if let Ok(mut records)=receipts.lock(){records.retain(|key,_|!key.starts_with(&format!("{}:",info.id)));}
            return json!({"ok":true});
        }
        let Some(exchange)=weak.upgrade() else { return json!({"ok":false,"error":"owning runtime stopped"}); };
        if request["action"]=="read" {
            let target=request["path"].as_str().unwrap_or_default();
            if !exchange.remote_operations.is_published(target){return json!({"ok":false,"error":"owner does not publish the readback path"});}
            let Ok(path)=MemberPath::parse(target) else{return json!({"ok":false,"error":"invalid readback path"});};
            return exchange.read_next(&[path],Duration::from_secs(2)).map_or_else(||json!({"ok":false,"error":"owner readback unavailable"}),|read|json!({"ok":true,"value":read.values.first().and_then(|value|value.as_ref())}));
        }
        let id=request["id"].as_str().unwrap_or_default();
        if id.is_empty() || id.len()>128 { return json!({"ok":false,"error":"command identity must have 1 to 128 characters"}); }
        let target=request["path"].as_str().unwrap_or_default();
        let fingerprint=json!({"path":target,"write":request["write"],"pulse_ms":request["pulse_ms"]});
        let key=format!("{}:{id}",info.id);
        let Ok(mut records)=receipts.lock() else { return json!({"ok":false,"error":"owner command receipts unavailable"}); };
        records.retain(|_,(_,_,expires)|*expires>Instant::now());
        if let Some((before,answer,_))=records.get(&key) { return if before==&fingerprint {answer.clone()} else {json!({"ok":false,"error":"command identity reused with different input"})}; }
        if records.len()>=4096 { return json!({"ok":false,"error":"owner remote command receipt limit reached; log out unused owner sessions"}); }
        let answer=(||->Result<Json,String>{
            if request["action"]!="write" { return Err("unsupported remote HMI operation".into()); }
            let path=MemberPath::parse(target)?;
            if crate::hmi::remote_values::remote_path(&path).is_some() || !exchange.remote_operations.is_published(target) { return Err("the target is not published by this owning runtime".into()); }
            let rule=rule_for(&access.write,&path).ok_or("the owning runtime write policy refuses this target")?;
            let permission=Permission{role:rule.role,min:rule.min,max:rule.max};
            authorize(&permission,info.role)?;
            let data=&request["write"];
            if data.as_object().is_none_or(|fields|fields.len()!=1) {return Err("provide exactly one native write action".into());}
            let action=if let Some(value)=data.get("set") {
                if let Some(number)=value.as_f64(){within_limits(&permission,number)?;}
                WriteAction::Set(value.clone())
            } else if data["toggle"]==true {WriteAction::Toggle}
            else if let (Some(name),Some(index))=(data["choice"]["name"].as_str(),data["choice"]["index"].as_u64()) {
                within_limits(&permission,index as f64)?;
                WriteAction::Choice{name:name.to_string(),index:usize::try_from(index).map_err(|_|"choice index is too large")?}
            } else {return Err("unsupported native write action".into());};
            let pulse=match request.get("pulse_ms") {
                None|Some(Json::Null)=>None,
                Some(value)=>Some(value.as_u64().ok_or("pulse_ms must be an unsigned integer")?),
            };
            if pulse.is_some() && action != WriteAction::Set(json!(true)) {return Err("a pulse must set a BOOL true".into());}
            if pulse.is_some_and(|ms|ms==0 || ms>60_000) {return Err("remote pulses must last between 1 ms and 60 seconds".into());}
            let result=exchange.write_with_pulse(path.clone(),action,Duration::from_secs(2),pulse.map(Duration::from_millis));
            let Some(written)=result else { return Ok(json!({"ok":false,"unknown":true,"error":"owner write outcome was not available before its deadline"})); };
            let written=written.map_err(|refused|refused.reason)?;
            Ok(json!({"ok":true,"old":json_of(&written.old),"readback":json_of(&written.readback)}))
        })().unwrap_or_else(|reason|json!({"ok":false,"error":reason}));
        if let Some(audit)=&audit {
            let _=audit.send(ControlAuditEvent{event_id:format!("hmi-remote-{}-{id}",info.id).into(),timestamp_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),request_id:0,request_type:"hmi.remote.command".into(),correlation_id:Some(id.into()),ok:answer["ok"]==true,error:answer["error"].as_str().map(Into::into),auth_present:true,client:Some("authenticated-mesh-hmi".into()),details:Some(json!({"user":info.user.as_str(),"session":info.id.as_str(),"target":target,"requested":request["write"],"old":answer["old"],"readback":answer["readback"],"unknown":answer["unknown"]}))});
        }
        records.insert(key,(fingerprint,answer.clone(),Instant::now()+info.expires_in));
        answer
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::hmi::access::config::{HmiUser, WriteRule};
    use crate::hmi::access::password::{hash_password, PasswordHash};
    use crate::security::AccessRole;
    #[test]
    fn owner_checks_named_session_limits_publication_and_replay_before_native_write() {
        let mut plant = TestHarness::from_source(
            "PROGRAM Main VAR Setpoint : REAL := 4.0; END_VAR END_PROGRAM",
        )
        .unwrap();
        plant.cycle();
        let exchange = Arc::new(HmiExchange::new());
        exchange
            .remote_operations
            .publish(vec!["Main.Setpoint".into()]);
        let access = Arc::new(HmiAccessConfig {
            users: vec![HmiUser {
                name: "operator".into(),
                role: AccessRole::Operator,
                password: PasswordHash::parse(&hash_password(
                    "secret",
                    10_000,
                    b"remote-test-salt",
                ))
                .unwrap(),
            }],
            write: vec![WriteRule {
                path: MemberPath::parse("Main.Setpoint").unwrap(),
                below: false,
                role: AccessRole::Operator,
                min: Some(0.0),
                max: Some(10.0),
            }],
            ..Default::default()
        });
        let sessions = Arc::new(SessionStore::new((*access).clone()));
        let (audit_tx, audit_rx) = std::sync::mpsc::channel();
        configure_owner(&exchange, sessions, access, Some(audit_tx));
        assert_eq!(
            exchange
                .remote_operations
                .receive(&json!({"action":"write","id":"a"}))["login_required"],
            true
        );
        let login = exchange
            .remote_operations
            .receive(&json!({"action":"login","user":"operator","password":"secret"}));
        let token = login["token"].as_str().unwrap();
        let denied=exchange.remote_operations.receive(&json!({"action":"write","token":token,"id":"denied","path":"Main.Setpoint","write":{"set":11}}));
        assert_eq!(denied["ok"], false);
        assert!(denied["error"].as_str().unwrap().contains("highest"));
        let request = json!({"action":"write","token":token,"id":"once","path":"Main.Setpoint","write":{"set":5}});
        let copy = exchange.clone();
        let input = request.clone();
        let writer = std::thread::spawn(move || copy.remote_operations.receive(&input));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !writer.is_finished() && std::time::Instant::now() < deadline {
            plant
                .runtime_mut()
                .with_program(|program| exchange.serve(program, 0, &[]));
            std::thread::yield_now();
        }
        let answer = writer.join().unwrap();
        assert_eq!(answer["ok"], true, "{answer}");
        assert_eq!(exchange.remote_operations.receive(&request), answer);
        let mut changed = request.clone();
        changed["write"] = json!({"set":6});
        assert_eq!(exchange.remote_operations.receive(&changed)["ok"], false);
        let events: Vec<_> = audit_rx.try_iter().collect();
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| !format!("{:?}", event.details).contains(token)));
    }
}
