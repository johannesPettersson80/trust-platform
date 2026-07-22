use super::*;

#[test]
fn add_notification_accepts_supported_modes_and_delete_releases_handle() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let response = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERONCHA, 4),
        &ctx,
    );

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[4..8].try_into().expect("handle"));
    assert_eq!(dispatcher.notification_count(), 1);
    assert_eq!(
        dispatcher.active_notifications()[0].transmission_mode,
        ADSTRANS_SERVERONCHA
    );
    assert_eq!(
        dispatcher.active_notifications()[0].receiver,
        NotificationReceiver {
            net_id: [1, 2, 3, 4, 5, 6],
            port: 0x8001,
        }
    );

    let response = dispatcher.dispatch(
        CommandId::DeleteDeviceNotification,
        &handle.to_le_bytes(),
        &ctx,
    );

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 0);
}

#[test]
fn add_notification_accepts_beckhoff_dotnet_v7_compact_request() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_000_000_u32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let handle = u32::from_le_bytes(response[4..8].try_into().expect("handle"));
    assert_ne!(handle, 0);
    assert_eq!(dispatcher.notification_count(), 1);
    assert_eq!(dispatcher.active_notifications()[0].cycle_time_ms, 100);
}

#[test]
fn add_notification_accepts_symbol_version_watch() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VERSION.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);
    let active = dispatcher.active_notifications();
    assert!(matches!(
        &active[0].target,
        NotificationTarget::SymbolVersion
    ));
}

#[test]
fn add_notification_accepts_symbol_value_handle_for_pyads() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(&13_u32.to_le_bytes());
    handle_request.extend_from_slice(b"GVL.Setpoint\0");
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
    request.extend_from_slice(&10_u32.to_le_bytes());
    request.extend_from_slice(&10_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);
}

#[test]
fn add_notification_accepts_online_change_count_handle_as_symbol_version() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{ONLINE_CHANGE_COUNT_NAME}\0");
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    handle_request.extend_from_slice(name.as_bytes());
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERONCHA.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let active = dispatcher.active_notifications();
    assert!(matches!(
        &active[0].target,
        NotificationTarget::SymbolVersion
    ));
}

#[test]
fn add_notification_accepts_task_count_handle_as_static_system_bytes() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let name = format!("{TASK_COUNT_NAME}\0");
    let mut handle_request = Vec::new();
    handle_request.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
    handle_request.extend_from_slice(&0_u32.to_le_bytes());
    handle_request.extend_from_slice(&4_u32.to_le_bytes());
    handle_request.extend_from_slice(
        &u32::try_from(name.len())
            .expect("test symbol name length fits u32")
            .to_le_bytes(),
    );
    handle_request.extend_from_slice(name.as_bytes());
    let handle_response = dispatcher.dispatch(CommandId::ReadWrite, &handle_request, &ctx);
    let handle = u32::from_le_bytes(handle_response[8..12].try_into().expect("handle response"));
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SYM_VALBYHND.to_le_bytes());
    request.extend_from_slice(&handle.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&[0; 16]);

    let response = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    let active = dispatcher.active_notifications();
    assert_eq!(
        active[0].target,
        NotificationTarget::SystemBytes(1_u32.to_le_bytes().to_vec())
    );
}

#[test]
fn add_notification_accepts_smaller_watch_and_rejects_too_large_watch() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let unsupported = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(99, 4),
        &ctx,
    );
    assert_eq!(
        u32::from_le_bytes(unsupported[0..4].try_into().expect("result")),
        AdsErrorCode::TransmissionModeNotSupported.value()
    );

    let smaller = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 2),
        &ctx,
    );
    assert_eq!(&smaller[0..4], &0_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 1);

    let too_large = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 5),
        &ctx,
    );
    assert_eq!(
        u32::from_le_bytes(too_large[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidWatchSize.value()
    );
}

#[test]
fn add_notification_enforces_per_client_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_max_notifications(1);

    let first = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 4),
        &ctx,
    );
    let second = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 4),
        &ctx,
    );

    assert_eq!(&first[0..4], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(second[0..4].try_into().expect("result")),
        AdsErrorCode::NoMemory.value()
    );
    assert_eq!(dispatcher.notification_count(), 1);
}

#[test]
fn add_notification_requires_receiver_without_allocating_handle() {
    let host = FakeHost::new();
    let client = test_client();
    let mut ctx = host.context(&client);
    ctx.notification_receiver = None;
    let mut dispatcher = CommandDispatcher::new();

    let response = dispatcher.dispatch(
        CommandId::AddDeviceNotification,
        &add_notification_request(ADSTRANS_SERVERCYCLE, 4),
        &ctx,
    );

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::ServiceNotSupported.value()
    );
    assert_eq!(dispatcher.notification_count(), 0);
}

#[test]
fn delete_notification_rejects_unknown_handle_without_mutation() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();

    let response = dispatcher.dispatch(
        CommandId::DeleteDeviceNotification,
        &99_u32.to_le_bytes(),
        &ctx,
    );

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::NotificationHandleInvalid.value()
    );
    assert_eq!(dispatcher.notification_count(), 0);
}

#[test]
fn add_notification_wraps_without_replacing_live_handle() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let request = add_notification_request(ADSTRANS_SERVERCYCLE, 4);

    let first = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);
    dispatcher.set_next_notification_handle_for_test(u32::MAX);
    let max = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);
    let wrapped = dispatcher.dispatch(CommandId::AddDeviceNotification, &request, &ctx);

    assert_eq!(&first[4..8], &1_u32.to_le_bytes());
    assert_eq!(&max[4..8], &u32::MAX.to_le_bytes());
    assert_eq!(&wrapped[4..8], &2_u32.to_le_bytes());
    assert_eq!(dispatcher.notification_count(), 3);
}

#[test]
fn device_notification_payload_matches_wire_matrix() {
    let payload = build_device_notification_payload(&[NotificationStamp::new(
        0x1122_3344_5566_7788,
        vec![NotificationSample::new(132, 1.25_f32.to_le_bytes())],
    )])
    .expect("notification payload");

    assert_eq!(&payload[0..4], &28_u32.to_le_bytes());
    assert_eq!(&payload[4..8], &1_u32.to_le_bytes());
    assert_eq!(&payload[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&payload[16..20], &1_u32.to_le_bytes());
    assert_eq!(&payload[20..24], &132_u32.to_le_bytes());
    assert_eq!(&payload[24..28], &4_u32.to_le_bytes());
    assert_eq!(&payload[28..32], &1.25_f32.to_le_bytes());
}

#[test]
fn invalidated_notification_sample_has_zero_size_data() {
    let payload = build_device_notification_payload(&[NotificationStamp::new(
        0x9988_7766_5544_3322,
        vec![NotificationSample::invalidated(133)],
    )])
    .expect("notification payload");

    assert_eq!(&payload[0..4], &24_u32.to_le_bytes());
    assert_eq!(&payload[4..8], &1_u32.to_le_bytes());
    assert_eq!(&payload[16..20], &1_u32.to_le_bytes());
    assert_eq!(&payload[20..24], &133_u32.to_le_bytes());
    assert_eq!(&payload[24..28], &0_u32.to_le_bytes());
    assert_eq!(payload.len(), 28);
}

fn add_notification_request(mode: u32, byte_len: u32) -> Vec<u8> {
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&byte_len.to_le_bytes());
    request.extend_from_slice(&mode.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&[0_u8; 16]);
    request
}
