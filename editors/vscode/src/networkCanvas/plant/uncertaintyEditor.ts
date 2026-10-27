/**
 * An imperfection effect as the plant editor's draft holds it (specification 47 §11). Rust owns its
 * rules and vocabulary: the targets, modes and generators, each generator's fields, the presets,
 * the defaults of a new or duplicated effect, the order, the checks and the entries a plant saves.
 */
export interface PlantUncertaintyDraft {
  enabled: boolean;
  id: string;
  target: string;
  mode: string;
  generator: string;
  channel?: number;
  parameter?: string;
  start_s?: number;
  end_s?: number;
  minimum?: number;
  maximum?: number;
  value?: number;
  quantum?: number;
  mean?: number;
  standard_deviation?: number;
  correlation_time_s?: number;
  diffusion_per_sqrt_s?: number;
  rate_per_s?: number;
  duration_s?: number;
  duration_min_s?: number;
  duration_max_s?: number;
  dropout_policy?: string;
  replacement_value?: number;
  amplitude?: number;
  frequency_hz?: number;
  phase_rad?: number;
  bias?: number;
  width?: number;
}
