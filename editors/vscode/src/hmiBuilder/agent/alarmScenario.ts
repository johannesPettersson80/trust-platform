import { symbolInstance } from "../editor/placed";
import type { AlarmFrame, HmiPageModel } from "../drawing/model";
import type { AlarmMessage } from "../editor/messages";
import { DESIGN_NOW_MS } from "../editor/designHistory";
/** Read-only preview data. Sample paths are never PLC alarm identities or live commands. */
export function alarmScenario(declared: AlarmMessage[], models: HmiPageModel[], active: boolean): AlarmFrame {
  const instances = models.flatMap((model) =>
    model.object.flatMap((object) => {
      const instance = object.instance ?? (object.click?.faceplate ? symbolInstance(object) : undefined);
      return instance ? [instance] : [];
    }),
  );
  const definitions = declared?.length
    ? declared
    : [...new Set(instances)].map((instance) => ({
        path: `${instance}.__preview_alarm`,
        message: `Preview alarm — ${instance}`,
      }));
  const alarms = active
    ? definitions.slice(0, 12).map((alarm, index) => ({
        path: alarm.path,
        message: alarm.message,
        priority: index === 0 ? "high" : "medium",
        state: "active",
        active: true,
        unacknowledged: true,
        shelved: false,
        out_of_service: false,
        suppressed: false,
        first_out: index === 0,
        annunciating: true,
        raise_count: 1,
        since_ms: DESIGN_NOW_MS - 30000 - index * 1000,
        equipment: instances
          .filter((instance) => alarm.path.startsWith(instance + "."))
          .sort((a, b) => b.length - a.length)[0],
      }))
    : [];
  return {
    summary: {
      active: alarms,
      shelved: [],
      suppressed: [],
      out_of_service: [],
      unacknowledged: alarms.length,
      annunciating: alarms.length,
      newest: alarms[0] ?? null,
    },
  };
}
