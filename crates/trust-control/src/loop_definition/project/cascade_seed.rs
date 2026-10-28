//! The `cascade-pid` seed of a program's own law, decoded (specification 35, C6): the inner and the outer controller, each with the
//! variables of its three gains. Its rules are those of `validCascadeSeed`: the family is PI, the two controllers are different
//! instances, and no two of the six variables are the same. Whether the variables are bound parameters is checked against the loop's
//! parameter binding (`ControllerProject::check_binding`).

use serde::Deserialize;
use serde_json::{Map, Value};

use super::has_repeat;
use crate::tuning::project_seed::{CascadeGains, CascadePidSeed};
use crate::tuning::structure::Structure;

/// The fields of one controller of a cascade seed (C6). Every field is required: each cascade PI names all three of its gains.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CascadeGainsFields {
    instance: String,
    proportional_symbol: String,
    integral_symbol: String,
    derivative_symbol: String,
}

/// The fields of a `cascade-pid` seed other than its kind (C6). Unknown fields are refused.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CascadeFields {
    family: Structure,
    inner: CascadeGainsFields,
    outer: CascadeGainsFields,
}

/// A cascade seed (C6, `validCascadeSeed`): the family is PI, the two controllers are different instances, and no two of the six
/// variables are the same.
///
/// # Errors
/// The seed's fields do not decode; its family is not PI; the two controllers are not two different instances; or one variable holds
/// two gains.
pub(super) fn cascade_seed(object: &Map<String, Value>) -> Result<CascadePidSeed, String> {
    let mut fields = object.clone();
    fields.remove("kind");
    let fields: CascadeFields = crate::json::decode_value(Value::Object(fields))
        .map_err(|error| format!("controller.project.seed: {error}"))?;
    if fields.family != Structure::PI {
        return Err("controller.project.seed of a cascade must be of family PI".to_owned());
    }
    if fields.inner.instance.is_empty()
        || fields.outer.instance.is_empty()
        || fields.inner.instance == fields.outer.instance
    {
        return Err(
            "controller.project.seed of a cascade names two different controllers".to_owned(),
        );
    }
    let seed = CascadePidSeed {
        inner: gains_of(fields.inner),
        outer: gains_of(fields.outer),
    };
    if has_repeat(&seed.variables()) {
        return Err("controller.project.seed names one variable for two gains".to_owned());
    }
    Ok(seed)
}

/// One controller's fields as the seed holds them.
fn gains_of(fields: CascadeGainsFields) -> CascadeGains {
    CascadeGains {
        instance: fields.instance,
        proportional_symbol: fields.proportional_symbol,
        integral_symbol: fields.integral_symbol,
        derivative_symbol: fields.derivative_symbol,
    }
}
