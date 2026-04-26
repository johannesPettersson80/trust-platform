# Boiler Feedwater Alarm Classic

This example combines classic `HYST` and `FIFO_16` so a boiler feedwater alarm
can enqueue an alarm code for a supervisor.

Compare it with `examples/oscat_components_boiler_feedwater_alarm_components`.

## Pattern Shown

- Classic alarm block feeding a classic queue.
- Event-code queueing.
- ST test for alarm detection and FIFO readout.

## Validate

```bash
trust-runtime test --project examples/oscat_components_boiler_feedwater_alarm_classic
```
