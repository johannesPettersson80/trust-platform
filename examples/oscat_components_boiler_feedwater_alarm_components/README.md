# Boiler Feedwater Alarm Components

This example composes `HysteresisSwitch` and `DwordFifo16` so alarm detection
and alarm-code queueing are independent objects.

Compare it with `examples/oscat_components_boiler_feedwater_alarm_classic`.

## Pattern Shown

- Alarm object feeding an event queue object.
- Interface-based composition across two domains.
- ST test for detection, enqueue, and dequeue.

## Validate

```bash
trust-runtime test --project examples/oscat_components_boiler_feedwater_alarm_components
```
