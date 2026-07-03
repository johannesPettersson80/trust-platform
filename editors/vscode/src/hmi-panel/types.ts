export type ControlRequestHandler = (
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params?: unknown
) => Promise<unknown>;

type HmiWidgetLocation = {
  file: string;
  line: number;
  column: number;
};

export type HmiWidgetSchema = {
  id: string;
  path: string;
  label: string;
  data_type: string;
  access: string;
  writable: boolean;
  widget: string;
  source: string;
  page: string;
  group: string;
  order: number;
  unit?: string | null;
  min?: number | null;
  max?: number | null;
  section_title?: string | null;
  widget_span?: number | null;
  location?: HmiWidgetLocation;
};

type HmiProcessScaleSchema = {
  min: number;
  max: number;
  output_min: number;
  output_max: number;
};

export type HmiProcessBindingSchema = {
  selector: string;
  attribute: string;
  source: string;
  format?: string | null;
  map?: Record<string, string>;
  scale?: HmiProcessScaleSchema | null;
};

export type HmiSectionSchema = {
  title: string;
  span: number;
  widget_ids?: string[];
};

export type HmiPageSchema = {
  id: string;
  title: string;
  order: number;
  kind?: string;
  icon?: string | null;
  duration_ms?: number | null;
  svg?: string | null;
  svg_content?: string | null;
  signals?: string[];
  sections?: HmiSectionSchema[];
  bindings?: HmiProcessBindingSchema[];
};

export type HmiSchemaResult = {
  version: number;
  mode: string;
  read_only: boolean;
  resource: string;
  generated_at_ms: number;
  theme?: {
    style?: string;
    accent?: string;
    background?: string;
    surface?: string;
    text?: string;
  };
  pages: HmiPageSchema[];
  widgets: HmiWidgetSchema[];
};

export type HmiValuesResult = {
  connected: boolean;
  timestamp_ms: number;
  freshness_ms?: number | null;
  values: Record<string, { v: unknown; q: string; ts_ms: number }>;
};

export type HmiTrendPoint = {
  ts_ms: number;
  value: number;
  min: number;
  max: number;
  samples: number;
};

export type HmiTrendSeries = {
  id: string;
  label: string;
  unit?: string | null;
  points: HmiTrendPoint[];
};

export type HmiTrendResult = {
  connected: boolean;
  timestamp_ms: number;
  duration_ms: number;
  buckets: number;
  series: HmiTrendSeries[];
};

export type HmiAlarmRecord = {
  id: string;
  widget_id: string;
  path: string;
  label: string;
  state: string;
  acknowledged: boolean;
  raised_at_ms: number;
  last_change_ms: number;
  value: number;
  min?: number | null;
  max?: number | null;
};

export type HmiAlarmHistoryRecord = {
  id: string;
  widget_id: string;
  path: string;
  label: string;
  event: string;
  timestamp_ms: number;
  value: number;
};

export type HmiAlarmResult = {
  connected: boolean;
  timestamp_ms: number;
  active: HmiAlarmRecord[];
  history: HmiAlarmHistoryRecord[];
};

export type LayoutWidgetOverride = {
  label?: string;
  page?: string;
  group?: string;
  order?: number;
  widget?: string;
  unit?: string;
  min?: number;
  max?: number;
};
export type LayoutOverrides = Record<string, LayoutWidgetOverride>;

export type LayoutFile = {
  version: 1;
  widgets: LayoutOverrides;
  updated_at: string;
};
