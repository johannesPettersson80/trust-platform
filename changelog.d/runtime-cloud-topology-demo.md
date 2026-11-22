### Fixed

- The multi-host demos in `examples/runtime_cloud/multi_host_topology_demo` start again. Their
  runtimes are `runtime_a` to `runtime_e`, and `runtime_a_control`, `runtime_a_gateway` and
  `runtime_b_control` in the realistic demo: resource names that a program can declare. The
  bootstrap scripts give each project's program the profile's resource; before, every build
  refused the mismatch (`resource 'runtime-a' of runtime.toml is not the RESOURCE the program
  declares ('CommMultiDriverRes')`). `stop-5.sh` stops the runtimes by their process IDs only.
