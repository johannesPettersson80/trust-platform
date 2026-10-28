// `io.list`, `io.read`, `io.write`, `io.force` and `io.unforce` are answered by the shared request
// pipeline (`trust_runtime_services::live_values::io_requests`). The HMI Builder's `hmi.read` is
// answered by `handlers/hmi.rs`; the legacy HMI's requests are removed (specification 39 §17.2).
