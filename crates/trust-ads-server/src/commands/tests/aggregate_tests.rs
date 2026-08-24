use super::*;

#[test]
fn sumup_read_enforces_item_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_limits(usize::MAX, 1, usize::MAX, usize::MAX);
    let mut sub = Vec::new();
    for _ in 0..2 {
        sub.extend_from_slice(&0x4020_u32.to_le_bytes());
        sub.extend_from_slice(&0_u32.to_le_bytes());
        sub.extend_from_slice(&4_u32.to_le_bytes());
    }
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&16_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
}

#[test]
fn direct_write_enforces_write_byte_limit() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::with_limits(usize::MAX, usize::MAX, 2, usize::MAX);
    let mut request = Vec::new();
    request.extend_from_slice(&0x4020_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&4_u32.to_le_bytes());
    request.extend_from_slice(&2.5_f32.to_le_bytes());

    let response = dispatcher.dispatch(CommandId::Write, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
    assert!(host.writes.borrow().is_empty());
}

#[test]
fn sumup_read_keeps_good_items_when_one_fails() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x9999_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&16_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[12..16].try_into().expect("second result")),
        AdsErrorCode::InvalidGroup.value()
    );
    assert_eq!(&response[16..20], &1.25_f32.to_le_bytes());
}

#[test]
fn sumup_read_ex_returns_result_lengths_for_ads_rs_read_multi() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x9999_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READ_EX.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&20_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &20_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(&response[12..16], &4_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[16..20].try_into().expect("second result")),
        AdsErrorCode::InvalidGroup.value()
    );
    assert_eq!(&response[20..24], &0_u32.to_le_bytes());
    assert_eq!(&response[24..28], &1.25_f32.to_le_bytes());
}

#[test]
fn sumup_write_returns_per_item_results() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&1_u32.to_le_bytes());
    sub.extend_from_slice(&1.0_f32.to_le_bytes());
    sub.push(0);
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_WRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&8_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(response[12..16].try_into().expect("second result")),
        AdsErrorCode::InvalidAccess.value()
    );
    assert_eq!(host.writes.borrow().len(), 1);
}

#[test]
fn sumup_readwrite_returns_metadata_then_concatenated_data() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    sub.extend_from_slice(&0x4020_u32.to_le_bytes());
    sub.extend_from_slice(&4_u32.to_le_bytes());
    sub.extend_from_slice(&1_u32.to_le_bytes());
    sub.extend_from_slice(&0_u32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READWRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&21_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read/write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(&response[0..4], &0_u32.to_le_bytes());
    assert_eq!(&response[4..8], &21_u32.to_le_bytes());
    assert_eq!(&response[8..12], &0_u32.to_le_bytes());
    assert_eq!(&response[12..16], &4_u32.to_le_bytes());
    assert_eq!(&response[16..20], &0_u32.to_le_bytes());
    assert_eq!(&response[20..24], &1_u32.to_le_bytes());
    assert_eq!(&response[24..28], &1.25_f32.to_le_bytes());
    assert_eq!(response[28], 1);
}

#[test]
fn sumup_write_rejects_truncated_later_item_before_any_write() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    for _ in 0..2 {
        sub.extend_from_slice(&0x4020_u32.to_le_bytes());
        sub.extend_from_slice(&0_u32.to_le_bytes());
        sub.extend_from_slice(&4_u32.to_le_bytes());
    }
    sub.extend_from_slice(&2.5_f32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_WRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&8_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
    assert!(host.writes.borrow().is_empty());
}

#[test]
fn sumup_readwrite_rejects_truncated_later_item_before_any_write() {
    let host = FakeHost::new();
    let client = test_client();
    let ctx = host.context(&client);
    let mut dispatcher = CommandDispatcher::new();
    let mut sub = Vec::new();
    for _ in 0..2 {
        sub.extend_from_slice(&0x4020_u32.to_le_bytes());
        sub.extend_from_slice(&0_u32.to_le_bytes());
        sub.extend_from_slice(&4_u32.to_le_bytes());
        sub.extend_from_slice(&4_u32.to_le_bytes());
    }
    sub.extend_from_slice(&2.5_f32.to_le_bytes());
    let mut request = Vec::new();
    request.extend_from_slice(&ADSIGRP_SUMUP_READWRITE.to_le_bytes());
    request.extend_from_slice(&2_u32.to_le_bytes());
    request.extend_from_slice(&24_u32.to_le_bytes());
    request.extend_from_slice(
        &u32::try_from(sub.len())
            .expect("test sum-up read/write payload fits u32")
            .to_le_bytes(),
    );
    request.extend_from_slice(&sub);

    let response = dispatcher.dispatch(CommandId::ReadWrite, &request, &ctx);

    assert_eq!(
        u32::from_le_bytes(response[0..4].try_into().expect("result")),
        AdsErrorCode::InvalidSize.value()
    );
    assert!(host.writes.borrow().is_empty());
}
