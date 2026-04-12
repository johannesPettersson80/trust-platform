pub fn run_bench(action: BenchAction) -> anyhow::Result<()> {
    let (report, output_format) = execute_bench(action)?;
    let rendered = render_bench_output(&report, output_format)?;
    print!("{rendered}");
    Ok(())
}

fn execute_bench(action: BenchAction) -> anyhow::Result<(BenchReport, BenchOutputFormat)> {
    match action {
        BenchAction::Project {
            project,
            samples,
            warmup_cycles,
            watch,
            output,
        } => {
            let workload = ProjectBenchWorkload::normalize(project, samples, warmup_cycles, watch)?;
            Ok((run_project_bench(workload)?, output))
        }
        BenchAction::T0Shm {
            samples,
            payload_bytes,
            output,
        } => {
            let workload = BenchWorkload::normalize(samples, payload_bytes)?;
            Ok((run_t0_shm_bench(workload)?, output))
        }
        BenchAction::MeshZenoh {
            samples,
            payload_bytes,
            loss_rate,
            reorder_rate,
            output,
        } => {
            let workload =
                MeshBenchWorkload::normalize(samples, payload_bytes, loss_rate, reorder_rate)?;
            Ok((run_mesh_zenoh_bench(workload)?, output))
        }
        BenchAction::Dispatch {
            samples,
            payload_bytes,
            fanout,
            output,
        } => {
            let workload = DispatchBenchWorkload::normalize(samples, payload_bytes, fanout)?;
            Ok((run_dispatch_bench(workload)?, output))
        }
        BenchAction::ExecutionBackend {
            samples,
            warmup_cycles,
            output,
        } => {
            let workload = ExecutionBackendBenchWorkload::normalize(samples, warmup_cycles)?;
            Ok((run_execution_backend_bench(workload)?, output))
        }
    }
}
