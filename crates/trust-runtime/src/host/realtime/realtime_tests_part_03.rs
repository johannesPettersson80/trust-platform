    #[test]
    fn t0_publish_and_read_reject_uninitialized_or_unpinned_channel_state() {
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

        transport.inject_unpinned_channel("line-a");

        let publish = transport
            .publish_hardrt(pub_handle, &[1, 2, 3, 4])
            .expect_err("unpinned channel must reject publish");
        assert_eq!(publish.code, T0ErrorCode::TransportFailure);

        let mut out = [0_u8; 4];
        let read = transport
            .read_hardrt(sub_handle, &mut out)
            .expect_err("unpinned channel must reject read");
        assert_eq!(read.code, T0ErrorCode::TransportFailure);
    }

    #[test]
    fn t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget() {
        let mut scheduler = T0CycleScheduler::new(T0SchedulerPolicy {
            max_cloud_ops_per_cycle: 3,
        });
        scheduler.begin_cycle(7);

        let out_of_order = scheduler
            .mark_exchange_point(T0ExchangePoint::PostTask)
            .expect_err("post-task before pre-task must fail");
        assert_eq!(out_of_order.code, T0ErrorCode::ContractViolation);

        scheduler
            .mark_exchange_point(T0ExchangePoint::PreTask)
            .expect("mark pre-task");
        scheduler
            .mark_exchange_point(T0ExchangePoint::PostTask)
            .expect("mark post-task");
        assert_eq!(scheduler.exchange_points_seen(), (true, true));

        let first = scheduler.consume_cloud_budget(2);
        let second = scheduler.consume_cloud_budget(3);
        assert_eq!(first, 2);
        assert_eq!(second, 1);
        assert_eq!(scheduler.denied_cloud_ops_total(), 2);
    }

    #[test]
    fn t0_scheduler_budget_isolated_under_cloud_stress_across_cycles() {
        let mut scheduler = T0CycleScheduler::new(T0SchedulerPolicy {
            max_cloud_ops_per_cycle: 1,
        });
        let mut granted = 0_u32;
        for cycle in 0..10_u64 {
            scheduler.begin_cycle(cycle);
            scheduler
                .mark_exchange_point(T0ExchangePoint::PreTask)
                .expect("mark pre");
            scheduler
                .mark_exchange_point(T0ExchangePoint::PostTask)
                .expect("mark post");
            granted = granted.saturating_add(scheduler.consume_cloud_budget(50));
        }
        assert_eq!(granted, 10, "cloud load should be clamped per cycle");
        assert_eq!(scheduler.denied_cloud_ops_total(), 490);
    }
