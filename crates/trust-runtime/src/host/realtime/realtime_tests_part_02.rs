    #[test]
    fn t0_bind_rejects_non_t0_route_and_denies_fallback() {
        let mut transport = transport_fixture();
        let err = transport
            .bind_publisher("line-a", RealtimeRoute::MeshIp, "sha256:feedface", 4, true)
            .expect_err("mesh/ip route must be rejected for T0 bind");
        assert_eq!(err.code, T0ErrorCode::ContractViolation);
        assert!(
            err.message.contains("non-HardRT"),
            "diagnostic should explicitly reject generic IP as HardRT"
        );
        assert_eq!(transport.fallback_denied_total(), 1);
        assert_eq!(
            transport
                .channel_counters("line-a")
                .expect("channel counters")
                .fallback_denied_count,
            1
        );
    }

    #[test]
    fn qos_tier_route_legality_matrix_matches_contract() {
        assert!(QosTier::T0HardRt.route_is_legal(RealtimeRoute::T0HardRt));
        assert!(!QosTier::T0HardRt.route_is_legal(RealtimeRoute::MeshIp));

        assert!(!QosTier::T1Fast.route_is_legal(RealtimeRoute::T0HardRt));
        assert!(QosTier::T1Fast.route_is_legal(RealtimeRoute::MeshIp));

        assert!(!QosTier::T2Ops.route_is_legal(RealtimeRoute::T0HardRt));
        assert!(QosTier::T2Ops.route_is_legal(RealtimeRoute::MeshIp));

        assert!(!QosTier::T3Diag.route_is_legal(RealtimeRoute::T0HardRt));
        assert!(QosTier::T3Diag.route_is_legal(RealtimeRoute::MeshIp));
    }

    #[test]
    fn t0_error_codes_map_to_canonical_comms_contract_codes() {
        assert_eq!(
            CommsErrorCode::from(T0ErrorCode::NotConfigured),
            CommsErrorCode::NotConfigured
        );
        assert_eq!(
            CommsErrorCode::from(T0ErrorCode::ContractViolation),
            CommsErrorCode::RtContractViolation
        );
        assert_eq!(
            CommsErrorCode::RtContractViolation.remediation_hint(),
            "Use pre-bound T0 handles and fixed-layout payloads; generic IP mesh is non-HardRT."
        );
    }

    #[test]
    fn t0_bind_enforces_schema_hash_and_fixed_layout_contract() {
        let mut transport = transport_fixture();
        let mismatch = transport
            .bind_publisher("line-a", RealtimeRoute::T0HardRt, "sha256:wrong", 4, true)
            .expect_err("schema mismatch must be rejected");
        assert_eq!(mismatch.code, T0ErrorCode::SchemaMismatch);

        let variable = transport
            .bind_publisher(
                "line-a",
                RealtimeRoute::T0HardRt,
                "sha256:feedface",
                4,
                false,
            )
            .expect_err("variable layout must be rejected");
        assert_eq!(variable.code, T0ErrorCode::ContractViolation);
    }

    #[test]
    fn t0_publish_and_read_track_overrun_and_latest_payload() {
        let mut transport = transport_fixture();
        let pub_handle = transport
            .bind_publisher(
                "line-a",
                RealtimeRoute::T0HardRt,
                "sha256:feedface",
                4,
                true,
            )
            .expect("bind publisher");
        let sub_handle = transport
            .bind_subscriber(
                "line-a",
                RealtimeRoute::T0HardRt,
                "sha256:feedface",
                4,
                true,
            )
            .expect("bind subscriber");
        transport
            .publish_hardrt(pub_handle, &[1, 2, 3, 4])
            .expect("first publish");
        transport
            .publish_hardrt(pub_handle, &[9, 8, 7, 6])
            .expect("second publish");

        let mut out = [0_u8; 4];
        let read = transport
            .read_hardrt(sub_handle, &mut out)
            .expect("read latest");
        assert_eq!(out, [9, 8, 7, 6]);
        match read {
            T0ReadOutcome::Fresh(details) => {
                assert_eq!(details.dropped_updates, 1);
                assert_eq!(details.overrun_count, 1);
            }
            T0ReadOutcome::NoUpdate => panic!("expected fresh read"),
        }
        assert_eq!(
            transport
                .channel_counters("line-a")
                .expect("channel counters")
                .overrun_count,
            1
        );
    }

    #[test]
    fn t0_read_surfaces_stale_after_bounded_misses_and_spin_limit() {
        let mut transport = transport_fixture();
        let pub_handle = transport
            .bind_publisher(
                "line-a",
                RealtimeRoute::T0HardRt,
                "sha256:feedface",
                4,
                true,
            )
            .expect("bind publisher");
        let sub_handle = transport
            .bind_subscriber(
                "line-a",
                RealtimeRoute::T0HardRt,
                "sha256:feedface",
                4,
                true,
            )
            .expect("bind subscriber");

        let mut out = [0_u8; 4];
        let first = transport
            .read_hardrt(sub_handle, &mut out)
            .expect("first miss should be bounded no-update");
        assert_eq!(first, T0ReadOutcome::NoUpdate);

        let stale = transport
            .read_hardrt(sub_handle, &mut out)
            .expect_err("second miss should surface stale");
        assert_eq!(stale.code, T0ErrorCode::StaleData);

        transport
            .publish_hardrt(pub_handle, &[1, 1, 1, 1])
            .expect("fresh publish");
        transport.inject_unstable_writer("line-a", 4);
        let spin = transport
            .read_hardrt(sub_handle, &mut out)
            .expect_err("unstable writer beyond bounded spin must fail stale");
        assert_eq!(spin.code, T0ErrorCode::StaleData);

        let counters = transport
            .channel_counters("line-a")
            .expect("channel counters");
        assert_eq!(counters.stale_count, 2);
        assert_eq!(counters.spin_exhausted_count, 1);
    }

