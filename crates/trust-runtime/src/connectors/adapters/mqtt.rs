//! MQTT status adapter home.

use super::super::mapping::{
    mqtt_session_status, ConnectorStatusProjection, MqttSessionProjection,
};

/// Project MQTT session and freshness status.
#[must_use]
pub fn project_mqtt_session(state: MqttSessionProjection) -> ConnectorStatusProjection {
    mqtt_session_status(state)
}
