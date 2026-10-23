//! The Settings dialog's route (specification 39, §21.4; Plant §10): `POST /api/hmi/settings`.
//!
//! An Engineer reads the settings of the instance a panel is opened for, as the component
//! definition chosen for the instance's type describes them, and applies changes. Every value is
//! checked against its description and its variable's type, the current values must still be the
//! ones the dialog showed, and all changes are written at one boundary between scans, or none.
//! Each write is audited. There is no provider, revision or request number.

use super::*;
use crate::hmi::access::policy::require_role;
use crate::hmi::exchange::{SettingChange, SettingsJob, SettingsOutcome};
use crate::hmi::page_files::HmiSetting;
use crate::hmi::setting_values::{requested_write, shown_setting, ShownSetting};
use crate::security::AccessRole;
use crate::value::Value;

/// The refusal when a value changed since the dialog read it.
const CHANGED: &str = "Changed meanwhile: refresh";

/// A request of the dialog.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::web) struct SettingsRequest {
    /// The panel's popup, `faceplate:<page>/<object id>`.
    faceplate: String,
    /// `read` or `apply`.
    action: String,
    /// The values to write, by member, in each setting's unit.
    #[serde(default)]
    changes: serde_json::Map<String, Json>,
    /// The values the dialog showed for the changed members.
    #[serde(default)]
    expected: serde_json::Map<String, Json>,
}

/// Why a request is refused, the setting it is about, and whether the values changed meanwhile.
struct Refusal {
    error: String,
    field: Option<String>,
    changed: bool,
}

impl Refusal {
    fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            field: None,
            changed: false,
        }
    }

    fn about(error: impl Into<String>, member: &str) -> Self {
        Self {
            field: Some(member.to_string()),
            ..Self::new(error)
        }
    }
}

/// The instance a panel is opened for, and its settings' descriptions.
struct Target {
    instance: String,
    settings: Vec<HmiSetting>,
}

/// What the dialog reads at one boundary: each setting's member, then the members its `$Member`
/// units name.
struct Reading {
    paths: Vec<MemberPath>,
    /// For each setting, where its unit's member is among `paths`.
    units: Vec<Option<usize>>,
}

impl Commands {
    pub(in crate::web) fn settings(
        &self,
        request: SettingsRequest,
        session: Option<SessionInfo>,
    ) -> Json {
        let result = self.settings_answer(&request, session.as_ref());
        match result {
            Ok(answer) => answer,
            Err(refusal) => {
                if let (Some(session), "apply") = (session.as_ref(), request.action.as_str()) {
                    let answer = CommandAnswer::refused(&request.faceplate, refusal.error.clone());
                    send_audit(
                        &self.control_state,
                        "hmi.settings",
                        Some(session),
                        &answer,
                        json!({
                            "faceplate": request.faceplate,
                            "target": refusal.field,
                            "requested": request.changes,
                        }),
                    );
                }
                let mut answer = json!({ "ok": false, "error": refusal.error });
                if let Some(field) = refusal.field {
                    answer["field"] = json!(field);
                }
                if refusal.changed {
                    answer["changed"] = json!(true);
                }
                answer
            }
        }
    }

    fn settings_answer(
        &self,
        request: &SettingsRequest,
        session: Option<&SessionInfo>,
    ) -> Result<Json, Refusal> {
        let session = session.ok_or_else(|| Refusal::new("log in to change settings"))?;
        // Engineers only (§21.4 item 1).
        require_role(AccessRole::Engineer, session.role).map_err(Refusal::new)?;
        let target = self.settings_target(&request.faceplate)?;
        match request.action.as_str() {
            "read" => {
                if !request.changes.is_empty() || !request.expected.is_empty() {
                    return Err(Refusal::new("a read takes no changes"));
                }
                let values = self.settings_job(&target, Vec::new())?;
                Ok(answer(&target, values))
            }
            "apply" => self.apply_settings(request, session, &target),
            _ => Err(Refusal::new("the action is read or apply")),
        }
    }

    /// The instance the panel the request names is opened for, and the settings of the
    /// component definition chosen for the instance's type.
    fn settings_target(&self, address: &str) -> Result<Target, Refusal> {
        let state = self.control_state.clone();
        let type_of = move |path: &str| instance_type(&state, path);
        let found = self.store.read(|root| {
            HmiFiles::new(root.to_path_buf()).faceplate_with_definition(address, &type_of)
        });
        let (faceplate, pair) = match found {
            Some(Ok(found)) => found,
            Some(Err(_)) => return Err(Refusal::new("the panel cannot be loaded")),
            None => return Err(Refusal::new("no object of the project opens that panel")),
        };
        if !faceplate.settings {
            return Err(Refusal::new("this panel has no settings"));
        }
        let instance = faceplate
            .settings_instance
            .ok_or_else(|| Refusal::new("this panel is not opened for an instance"))?;
        let path = MemberPath::parse(&instance).map_err(Refusal::new)?;
        if crate::hmi::remote_values::remote_path(&path).is_some() {
            return Err(Refusal::new(
                "open the instance's own runtime to change its settings",
            ));
        }
        Ok(Target {
            instance,
            settings: pair.map(|pair| pair.settings).unwrap_or_default(),
        })
    }

    /// Checks the changes, then writes them all at one boundary, or none, and audits each.
    fn apply_settings(
        &self,
        request: &SettingsRequest,
        session: &SessionInfo,
        target: &Target,
    ) -> Result<Json, Refusal> {
        if request.changes.is_empty() {
            return Err(Refusal::new("there are no changes to apply"));
        }
        let before = self.settings_job(target, Vec::new())?;
        let mut changed = Vec::new();
        let mut changes = Vec::new();
        {
            let metadata = self
                .control_state
                .metadata
                .lock()
                .map_err(|_| Refusal::new("PLC metadata unavailable"))?;
            for (member, requested) in &request.changes {
                let index = target
                    .settings
                    .iter()
                    .position(|setting| setting.member.eq_ignore_ascii_case(member))
                    .ok_or_else(|| {
                        Refusal::about(format!("{member} is not a setting of this panel"), member)
                    })?;
                let setting = &target.settings[index];
                if changed.contains(&index) {
                    return Err(Refusal::about(
                        format!("{}: changed twice", setting.title),
                        &setting.member,
                    ));
                }
                let current = before.values[index].as_ref().ok_or_else(|| {
                    Refusal::about(
                        format!("{}: the value cannot be read", setting.title),
                        &setting.member,
                    )
                })?;
                let write = requested_write(metadata.registry(), setting, current, requested)
                    .map_err(|reason| Refusal::about(reason, &setting.member))?;
                changed.push(index);
                changes.push(SettingChange {
                    path: member_path(&target.instance, &setting.member)?,
                    unchanged: current.clone(),
                    write,
                });
            }
        }
        // The values must still be the ones the dialog showed (§21.4 item 4).
        for &index in &changed {
            let setting = &target.settings[index];
            let expected = request
                .expected
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(&setting.member))
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    Refusal::about(
                        format!("{}: send the value the change replaces", setting.title),
                        &setting.member,
                    )
                })?;
            if !matches(&before.shown[index].value, expected) {
                return Err(Refusal {
                    changed: true,
                    ..Refusal::about(CHANGED, &setting.member)
                });
            }
        }
        let after = self.settings_job(target, changes)?;
        for &index in &changed {
            let setting = &target.settings[index];
            let answer = CommandAnswer {
                readback: Some(after.shown[index].value.clone()),
                ..CommandAnswer::new(&request.faceplate, Outcome::Applied)
            };
            send_audit(
                &self.control_state,
                "hmi.settings",
                Some(session),
                &answer,
                json!({
                    "faceplate": request.faceplate,
                    "target": crate::hmi::page_files::pair_member_path(&target.instance, &setting.member),
                    "old": before.shown[index].value,
                    "requested": request
                        .changes
                        .iter()
                        .find(|(member, _)| member.eq_ignore_ascii_case(&setting.member))
                        .map(|(_, value)| value.clone()),
                    "readback": after.shown[index].value,
                }),
            );
        }
        Ok(answer(target, after))
    }

    /// Runs a settings job at the next boundary: `changes`, all or none, then every setting's
    /// value and unit read back.
    fn settings_job(
        &self,
        target: &Target,
        changes: Vec<SettingChange>,
    ) -> Result<Settings, Refusal> {
        let reading = reading(target)?;
        let outcome = crate::control::hmi_ports::hmi_exchange(&self.control_state).settings(
            SettingsJob {
                read: reading.paths.clone(),
                changes,
            },
            WRITE_TIMEOUT,
        );
        match outcome {
            Some(SettingsOutcome::Values(values)) => Ok(Settings::new(target, &reading, values)),
            Some(SettingsOutcome::Changed(path)) => {
                let member = path_text(&path);
                let setting = target
                    .settings
                    .iter()
                    .find(|setting| {
                        member.eq_ignore_ascii_case(&format!(
                            "{}.{}",
                            target.instance, setting.member
                        ))
                    })
                    .map_or(member.clone(), |setting| setting.member.clone());
                Err(Refusal {
                    changed: true,
                    ..Refusal::about(CHANGED, &setting)
                })
            }
            Some(SettingsOutcome::Refused(reason)) => Err(Refusal::new(reason)),
            None => Err(Refusal::new(
                "the runtime did not answer in time; refresh to see the values",
            )),
        }
    }
}

/// The settings as read at one boundary: each setting's value and its shown form.
struct Settings {
    values: Vec<Option<Value>>,
    shown: Vec<ShownSetting>,
}

impl Settings {
    fn new(target: &Target, reading: &Reading, mut values: Vec<Option<Value>>) -> Self {
        let shown = target
            .settings
            .iter()
            .zip(&reading.units)
            .enumerate()
            .map(|(index, (setting, unit))| {
                let unit = match unit {
                    Some(at) => match values.get(*at) {
                        Some(Some(Value::String(text))) => Some(text.to_string()),
                        _ => None,
                    },
                    None => setting.unit.clone(),
                };
                shown_setting(setting, unit, values.get(index).and_then(Option::as_ref))
            })
            .collect();
        values.truncate(target.settings.len());
        Self { values, shown }
    }
}

/// What the dialog reads: each setting's member, then the members its `$Member` units name.
fn reading(target: &Target) -> Result<Reading, Refusal> {
    let mut paths = target
        .settings
        .iter()
        .map(|setting| member_path(&target.instance, &setting.member))
        .collect::<Result<Vec<_>, _>>()?;
    let mut units = Vec::with_capacity(target.settings.len());
    for setting in &target.settings {
        units.push(
            match setting
                .unit
                .as_deref()
                .and_then(|unit| unit.strip_prefix('$'))
            {
                Some(member) => {
                    paths.push(member_path(&target.instance, member)?);
                    Some(paths.len() - 1)
                }
                None => None,
            },
        );
    }
    Ok(Reading { paths, units })
}

/// A member of the instance, or a variable beside it (`{instance}_Parameters.ProportionalGain`).
fn member_path(instance: &str, member: &str) -> Result<MemberPath, Refusal> {
    MemberPath::parse(&crate::hmi::page_files::pair_member_path(instance, member))
        .map_err(|error| Refusal::about(error, member))
}

/// The dialog's answer: the instance and its settings, with their values in their units.
fn answer(target: &Target, settings: Settings) -> Json {
    json!({ "ok": true, "instance": target.instance, "settings": settings.shown })
}
