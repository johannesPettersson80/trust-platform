    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_temp_path(label: &str) -> PathBuf {
        static TEST_NONCE: AtomicU64 = AtomicU64::new(0);
        let nonce = TEST_NONCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "trust-runtime-realtime-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn transport_fixture() -> T0Transport {
        let mut transport = T0Transport::new();
        transport
            .register_channel(
                "line-a",
                "sha256:feedface",
                T0ChannelPolicy {
                    slot_size: 8,
                    stale_after_reads: 2,
                    max_spin_retries: 2,
                    max_spin_time_us: 50,
                },
            )
            .expect("register channel");
        transport
    }

    #[test]
    fn t0_shm_registration_fails_fast_when_root_path_is_not_directory() {
        let root_path = unique_temp_path("root-file");
        std::fs::write(&root_path, b"not-a-directory").expect("create root file");
        let mut transport = T0Transport::with_config(T0ShmConfig::with_root(root_path.clone()));
        let err = transport
            .register_channel("line-a", "sha256:feedface", T0ChannelPolicy::default())
            .expect_err("registration must fail when SHM root is not a directory");
        assert_eq!(err.code, T0ErrorCode::TransportFailure);
        assert!(
            err.message.contains("failed to create T0 SHM directory"),
            "error should explain startup SHM directory failure"
        );
        let _ = std::fs::remove_file(root_path);
    }

    #[test]
    fn t0_shm_registration_fails_fast_when_required_pinning_is_unavailable() {
        let root_path = unique_temp_path("pin-required");
        let mut transport = T0Transport::with_config(T0ShmConfig {
            root_dir: root_path.clone(),
            pinning_mode: T0PinningMode::Required,
            pinning_provider: T0PinningProvider::None,
        });
        let err = transport
            .register_channel("line-a", "sha256:feedface", T0ChannelPolicy::default())
            .expect_err("registration must fail when required pinning cannot run");
        assert_eq!(err.code, T0ErrorCode::TransportFailure);
        assert!(
            err.message.contains("required page pinning failed"),
            "error should mention required pinning contract"
        );
        let _ = std::fs::remove_dir_all(root_path);
    }

    #[test]
    fn t0_shm_contract_mismatch_is_rejected_before_run() {
        let root_path = unique_temp_path("contract-mismatch");
        let mut producer = T0Transport::with_config(T0ShmConfig::with_root(root_path.clone()));
        producer
            .register_channel("line-a", "sha256:feedface", T0ChannelPolicy::default())
            .expect("register producer channel");

        let mut consumer = T0Transport::with_config(T0ShmConfig::with_root(root_path.clone()));
        let err = consumer
            .register_channel("line-a", "sha256:different", T0ChannelPolicy::default())
            .expect_err("schema mismatch must fail before RUN");
        assert_eq!(err.code, T0ErrorCode::TransportFailure);
        assert!(
            err.message.contains("schema_hash contract mismatch"),
            "registration should enforce SHM handshake contract"
        );
        let _ = std::fs::remove_dir_all(root_path);
    }

    #[test]
    fn t0_shm_header_fuzz_rejects_corruption_budget() {
        fn overwrite_mapping(path: &std::path::Path, bytes: &[u8]) {
            use std::io::{Seek, SeekFrom, Write};

            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .expect("open shm mapping for in-place mutation");
            let file_len = file.metadata().expect("stat shm mapping length").len() as usize;
            assert_eq!(
                file_len,
                bytes.len(),
                "test mutation must preserve mapping length"
            );
            file.seek(SeekFrom::Start(0))
                .expect("seek to shm mapping start");
            file.write_all(bytes).expect("write shm mapping bytes");
            file.flush().expect("flush shm mapping bytes");
        }

        fn next(state: &mut u64) -> u64 {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state
        }

        let iterations = std::env::var("TRUST_COMMS_FUZZ_ITERS")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(128);
        let root_path = unique_temp_path("header-fuzz");

        let mut producer = T0Transport::with_config(T0ShmConfig::with_root(root_path.clone()));
        producer
            .register_channel("line-a", "sha256:feedface", T0ChannelPolicy::default())
            .expect("register producer channel");

        let readiness = producer.shm_channel_readiness();
        let mapping_path = std::path::PathBuf::from(
            readiness
                .first()
                .expect("shm readiness entry")
                .mapping_path
                .clone(),
        );
        let original = std::fs::read(&mapping_path).expect("read shm mapping");
        // Windows denies truncating/replacing a file while a mapped section is open.
        drop(producer);

        let mut rng = 0xF00D_CAFE_1234_5678_u64;
        let header_offsets = [0_usize, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80];

        for _ in 0..iterations {
            let mut mutated = original.clone();
            let index = header_offsets[(next(&mut rng) as usize) % header_offsets.len()];
            let mask = (next(&mut rng) as u8) | 1;
            mutated[index] ^= mask;
            overwrite_mapping(&mapping_path, &mutated);

            let mut consumer = T0Transport::with_config(T0ShmConfig::with_root(root_path.clone()));
            let err = consumer
                .register_channel("line-a", "sha256:feedface", T0ChannelPolicy::default())
                .expect_err("mutated shm header must fail registration");
            assert_eq!(err.code, T0ErrorCode::TransportFailure);
        }

        overwrite_mapping(&mapping_path, &original);
        let _ = std::fs::remove_dir_all(root_path);
    }

    #[test]
    fn t0_shm_readiness_metadata_matches_meta_shm_channels_contract() {
        let transport = transport_fixture();
        let readiness = transport.shm_channel_readiness();
        assert_eq!(readiness.len(), 1);
        let channel = &readiness[0];
        assert_eq!(channel.channel_id, "line-a");
        assert_eq!(channel.schema_id, "line-a");
        assert_eq!(channel.schema_hash, "sha256:feedface");
        assert_eq!(channel.slot_size, 8);
        assert_eq!(channel.ownership, "publisher_writes");
        assert!(channel.pinned);
        assert!(channel.ready);

        let key = crate::runtime_cloud::keyspace::meta_shm_channels_key("site-a", "rt-1");
        assert_eq!(key, "truST/site-a/rt-1/_meta/shm_channels");

        let payload_json = transport
            .shm_channel_readiness_json()
            .expect("encode readiness json");
        let payload: serde_json::Value =
            serde_json::from_str(&payload_json).expect("decode readiness json");
        assert!(payload.is_array());
    }
