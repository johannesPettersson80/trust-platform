//! Specification 46 section 5.2: omitted state resolves to model-sized zeros.
use trust_runtime::simulation::{SimulationConfig, SimulationController};

#[test]
fn omitted_initial_state_resolves_to_exact_zero_dimensions_for_every_model() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "trust-omitted-plant-state-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let cases = [
        ("static_gain", "gain = 2.0", 0, 1),
        ("first_order", "time_constant_s = 7.0", 1, 1),
        ("first_order_dead_time", "time_constant_s = 7.0", 1, 1),
        ("integrator", "gain = 2.0", 1, 1),
        (
            "gravity_tank",
            "area_m2 = 2.0\noutflow_coefficient_m2_sqrt_m_per_s = 0.5",
            1,
            1,
        ),
        (
            "robot_joint",
            "inertia_kg_m2 = 2.0\nfriction_smoothing_rad_s = 0.01",
            2,
            2,
        ),
        (
            "second_order",
            "natural_frequency_rad_s = 2.0\ndamping_ratio = 0.7",
            2,
            1,
        ),
        (
            "piecewise_linear",
            "input_points = [0.0, 1.0]\noutput_points = [0.0, 1.0]",
            0,
            1,
        ),
        (
            "state_space",
            "a = [[0.0, 1.0], [-1.0, -1.0]]\nb = [[0.0], [1.0]]\nc = [[1.0, 0.0]]\nd = [[0.0]]",
            2,
            1,
        ),
    ];
    for (model, parameters, states, outputs) in cases {
        let file = directory.join(format!("{model}.toml"));
        let feedback = if outputs == 2 {
            "[\"%IL0\", \"%IL8\"]"
        } else {
            "[\"%IL0\"]"
        };
        std::fs::write(
            &file,
            format!(
                "[simulation]\nenabled = true\n[[plants]]\nid = \"process\"\nmodel = \"{model}\"\ncommand_sources = [\"%QL0\"]\nfeedback_targets = {feedback}\n[plants.parameters]\n{parameters}\n"
            ),
        )
        .unwrap();
        let config = SimulationConfig::load(&file)
            .unwrap_or_else(|error| panic!("{model} omitted state must be runtime-valid: {error}"));
        let controller = SimulationController::new(config);
        let status = controller.status();
        assert_eq!(status.plants.len(), 1, "{model}");
        assert_eq!(status.plants[0].state, vec![0.0; states], "{model}");
        assert_eq!(status.plants[0].ideal_output, vec![0.0; outputs], "{model}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}
