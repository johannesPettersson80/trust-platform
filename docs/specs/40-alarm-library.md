# Alarm library

> **Plant integration, 6 October 2026:** The Plant Library aggregates this existing alarm state machine through IAlarm/IAlarmSource. Alarm detection stays with equipment/application FBs; [trust-plant-library.md](trust-plant-library.md) defines the replacement public integration. This specification continues to own the underlying Alarm behavior.

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026. It follows the design agreed with
the user on 1 October 2026 (specification 39, §13 and decisions 13 and 14).

This specification defines the shipped `Alarm` library: function blocks for alarms in truST
programs, their lifecycle, and the runtime's alarm service that lists them, applies operator
actions and records every transition. The HMI display of alarms is specified in specification 39,
§13.6.

Alarm management follows ISA-18.2 (IEC 62682) and EEMUA 191. IEC 61131-3 does not define alarm
handling, so this is a truST library contract, neither an IEC standard function block nor an IEC
deviation. Its declarations use IEC 61131-3 Edition 3 Table 11 derived data types, Table 40
function blocks, Table 48 methods and inheritance (`EXTENDS`), and Tables 64 through 66
namespaces.

All public declarations belong to namespace `Alarm`. A project adds the library as a dependency
in its `trust-lsp.toml` (`Alarm = { path = "…/libraries/alarm", version = "0.1.0" }`) and writes
`USING Alarm;`.

## 1. Public surface

### 1.1 Datatypes

| Datatype | Values or fields |
|---|---|
| `AlarmPriority` | `Low := 1`, `Medium := 2`, `High := 3` |
| `AlarmState` | `Normal`, `ActiveUnacknowledged`, `ActiveAcknowledged`, `ClearedUnacknowledged` |
| `LimitDirection` | `High`, `Low` |
| `AlarmRequest` | operator actions waiting for the next scan (§4) |

### 1.2 Function blocks

| Function block | Purpose |
|---|---|
| `Alarm.Alarm` | The base: lifecycle, message, priority, delays, latching, shelving, suppression, out of service and first out. Detection blocks extend it. |
| `Alarm.DigitalAlarm` | Active while its `Condition` is TRUE. |
| `Alarm.LimitAlarm` | Active while a value is beyond a limit, with a deadband; high and high-high are two instances with their own priorities. |
| `Alarm.DeviationAlarm` | Active while two values differ by more than a limit, with a deadband. |
| `Alarm.DiscrepancyAlarm` | Active while a command and its feedback disagree, for example failed to start or failed to open. |
| `Alarm.FirstOutGroup` | Marks the first alarm of a group to become active, until the group is reset. |

## 2. `FUNCTION_BLOCK Alarm.Alarm`

### 2.1 Configuration inputs

| Input | Type | Default | Meaning |
|---|---|---|---|
| `Message` | `STRING[120]` | `''` | What the operator reads. |
| `Priority` | `AlarmPriority` | `Low` | ISA-18.2 and EEMUA 191: most alarms Low, few High. |
| `OnDelay` | `TIME` | `T#0s` | The condition must hold this long before the alarm becomes active. |
| `OffDelay` | `TIME` | `T#0s` | The condition must be gone this long before the alarm clears. |
| `Latching` | `BOOL` | `FALSE` | A latching alarm stays active until it is reset, even when its condition clears. |
| `Suppressed` | `BOOL` | `FALSE` | Suppressed by design: the program states that the alarm does not apply, for example while its unit is stopped. |
| `Group` | `REF_TO FirstOutGroup` | `NULL` | The first-out group the alarm belongs to, if any. |

### 2.2 Outputs

| Output | Type | Meaning |
|---|---|---|
| `Active` | `BOOL` | The alarm is active: its condition holds (after the delays), or it is latched. |
| `Unacknowledged` | `BOOL` | It waits for acknowledgement. |
| `State` | `AlarmState` | The ISA-18.2 state (§3.1). |
| `Shelved` | `BOOL` | An operator shelved it (§3.2). |
| `OutOfService` | `BOOL` | It is out of service (§3.4). |
| `SuppressedByDesign` | `BOOL` | The program suppresses it (§3.3). |
| `FirstOut` | `BOOL` | It became active first in its group since the group's last reset (§3.6). |
| `Annunciating` | `BOOL` | It is active or unacknowledged, and neither shelved, suppressed nor out of service: the output for horns and beacons. |
| `RaiseCount` | `UDINT` | How often it has become active; the runtime and nuisance-alarm reviews use it. |

### 2.3 Methods

| Method | Use |
|---|---|
| `Evaluate(Condition : BOOL)` | Protected. A detection block calls it once per scan with its raw condition; it applies waiting operator actions, the delays and the lifecycle. |
| `Acknowledge()` | For a local push button: acknowledges as an operator's acknowledgement does, without a user. |
| `Reset()` | Resets a latched alarm whose condition is gone. |

Normal use stays plain: the program calls the detection block every scan with its inputs and
reads its outputs. Shelving and out of service are operator actions through the runtime (§4), not
program methods.

### 2.4 Own detection

A customer block extends `Alarm.Alarm`, adds its own inputs and calls `Evaluate` in its body. It
gets the whole lifecycle, the records and the display (§5):

```iecst
FUNCTION_BLOCK DryRunAlarm EXTENDS Alarm.Alarm
VAR_INPUT
    Running : BOOL;
    Flow : REAL;
    MinimumFlow : REAL := 1.0;
END_VAR
    Evaluate(Condition := Running AND Flow < MinimumFlow);
END_FUNCTION_BLOCK
```

Calling a block that extends another runs only its own body (specification 04), so a detection
block that does not call `Evaluate` never becomes active.

## 3. Lifecycle

### 3.1 States (ISA-18.2)

| From | Event | To |
|---|---|---|
| `Normal` | the condition becomes active (after `OnDelay`) | `ActiveUnacknowledged` |
| `ActiveUnacknowledged` | acknowledged | `ActiveAcknowledged` |
| `ActiveUnacknowledged` | the condition clears (after `OffDelay`), not latching | `ClearedUnacknowledged` |
| `ActiveAcknowledged` | the condition clears, not latching | `Normal` |
| `ClearedUnacknowledged` | acknowledged | `Normal` |
| `ClearedUnacknowledged` | the condition becomes active again | `ActiveUnacknowledged` |

- An alarm that clears before it is acknowledged stays listed until it is acknowledged.
- Acknowledging an alarm that waits for no acknowledgement changes nothing.
- Each time the alarm becomes active, `RaiseCount` grows by one.

### 3.2 Shelved

- An operator shelves an alarm for a duration of 1 s to 24 h; it returns by itself when the time
  ends, or when an operator unshelves it.
- While shelved it does not annunciate: it is not in the active list and needs no
  acknowledgement. Its condition is still followed.
- On return, an alarm whose condition holds becomes `ActiveUnacknowledged`; otherwise `Normal`.

### 3.3 Suppressed by design

While `Suppressed` is TRUE the alarm does not annunciate, as when shelved. When `Suppressed`
becomes FALSE the alarm returns as from shelving.

### 3.4 Out of service

An engineer takes an alarm out of service for maintenance and puts it back. It does not annunciate
while out of service and returns as from shelving.

### 3.5 Latched

A latching alarm stays `Active` after its condition clears. It is reset by `Reset()` or an
operator's reset (§4), which take effect only when the condition is gone. After the reset it is
`Normal` if acknowledged, otherwise `ClearedUnacknowledged`.

### 3.6 First out

`FirstOutGroup` has one method, `Reset()`. Of the alarms whose `Group` refers to it, the first to
become active after the group's last reset has `FirstOut` TRUE; the others FALSE. It keeps
`FirstOut` until the group is reset, even after it clears. A trip's cause is a High alarm marked
first out; the trip itself is an event (specification 39, §13.3).

### 3.7 Detection blocks

| Block | Inputs | Active while |
|---|---|---|
| `DigitalAlarm` | `Condition : BOOL` | `Condition` is TRUE |
| `LimitAlarm` | `Value : LREAL`, `Limit : LREAL`, `Deadband : LREAL := 0`, `Direction : LimitDirection := High` | High: from `Value > Limit` until `Value < Limit - Deadband`. Low: from `Value < Limit` until `Value > Limit + Deadband`. |
| `DeviationAlarm` | `Value : LREAL`, `Reference : LREAL`, `Limit : LREAL`, `Deadband : LREAL := 0` | from `ABS(Value - Reference) > Limit` until it is below `Limit - Deadband` |
| `DiscrepancyAlarm` | `Command : BOOL`, `Feedback : BOOL` | `Command <> Feedback`; `OnDelay` is the time the equipment may take to follow |

A value that is not finite counts as beyond the limit: a broken measurement is not hidden.

### 3.8 Retained state

The alarm's state lives in its instance. Declared in a `VAR RETAIN` section, an instance keeps its
state, shelving and out-of-service marks across a warm restart (specification 03); without
`RETAIN` it starts `Normal`.

## 4. Operator actions

The runtime applies operator actions by writing the instance's `Request` member, which `Evaluate`
applies in the next scan and then clears:

| Action | Role | Request field | Value |
|---|---|---|---|
| Acknowledge | Operator | `Acknowledge : BOOL` | — |
| Shelve | Operator | `Shelve : TIME` | the duration, 1 s to 24 h |
| Unshelve | Operator | `Unshelve : BOOL` | — |
| Reset | Operator | `Reset : BOOL` | — |
| Comment | Operator | `Comment : STRING[120]` | the text |
| Out of service | Engineer | `RemoveFromService : BOOL` | — |
| In service | Engineer | `ReturnToService : BOOL` | — |

Each action is a command (specification 39, §12): it needs a session with the role, goes to the
audit log, and its outcome is confirmed when the instance shows the action applied (for example
`Unacknowledged` FALSE after an acknowledgement).

The alarm's state decides which actions it allows. One rule in the runtime decides both what the
display offers (specification 39, §5.7 item 4) and what the runtime accepts; it refuses any other
action, with its reason:
- Acknowledge, while unacknowledged, except while suppressed.
- Shelve, while in the active list, for one of 15 min, 1 h, 4 h, 8 h or 24 h on the display.
- Unshelve, while shelved.
- Reset, for a latching alarm that is active, while in the active list.
- Out of service, for an Engineer, while in the active or the shelved list.
- In service, for an Engineer, while out of service.
- Comment, always.

Without a session the display offers the Operator's actions, and choosing one asks to log in.

## 5. The runtime's alarm service

1. **Alarm list:** the runtime lists every instance of a block that extends `Alarm.Alarm`, at any
   depth of the program, so customer alarms appear without registration. An alarm's identity is
   its instance path, for example `Tank.LevelHigh`.
2. **Reading:** at each call of `Hmi.Display` (specification 39, §18 phase 5) it reads each
   alarm's outputs and derives the transitions since the last read: raised (one per `RaiseCount` step), cleared, acknowledged, shelved, unshelved,
   suppressed, unsuppressed, out of service, in service and reset.
3. **Records:** each transition becomes a record with the time, the instance path, the message,
   the priority and, for an operator action, the user:
   - with `[runtime.openot]` enabled, as an OpenOT condition record whose condition identity is
     derived from the instance path; the OpenOT records are the alarm history;
   - the latest 1 000 records, with the command outcomes of the audit log, are kept in memory for
     the event log (specification 39, §13.7); this is a view of recent events, not a second
     history.
4. **Restart:** after a program change or a cold restart the list is read again.
5. **Cost:** a program without the library is not searched; the list is found by one walk of the
   program's instances per read, and each alarm's outputs are read from its instance directly.
6. **Owner:** the runtime keeps the alarm list and the records, updated at each call of
   `Hmi.Display` whether or not a screen is open or its web server runs, so the history does not
   depend on a display; a program without the call has no alarm list and no records. The web
   server reads them without waiting on the program, and passes each operator action's user to
   the runtime.

### 5.2 OpenOT records

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026; the numbers marked † are proposals
for the OpenOT working group to ratify.

With `[runtime.openot]` enabled, the runtime publishes each transition of §5, item 2, to the
OpenOT ring beside the records it already publishes:

| Transition | OpenOT event |
|---|---|
| raised | `ConditionActive` (0x0200) |
| cleared | `ConditionCleared` (0x0201) |
| acknowledged | `ConditionAcknowledged` (0x0202) |
| shelved, unshelved | `ConditionShelved`, `ConditionUnshelved` (0x0204, 0x0205) |
| suppressed, unsuppressed | `ConditionSuppressed`, `ConditionUnsuppressed` (0x0206, 0x0207) |
| out of service, in service | `ConditionOutOfService`, `ConditionInService` (0x0208, 0x0209) |
| comment | `ConditionCommented` (0x020A) |
| reset | `ConditionReset` (0x020B) |

- **Source:** the alarm service is one OpenOT source, `source_id` 61 440 (0xF000)†, apart from
  the identities authored programs receive from 1 (specification 27, §10.1). Its `run_id` is the
  runtime's start time in milliseconds and changes with every start; its `seq` counts its records
  from 1; `source_time` is the transition's time in nanoseconds since the Unix epoch.
- **Identity:** the condition's identity is its instance path. Every record carries the path as
  `SourcePath` (key 0x001B, a string) and `ConditionId` (key 0x0005, an unsigned 32-bit number):
  the 32-bit FNV-1a hash of the path in lower case†, 1 in place of 0, which ConditionId may not
  be.
- **Per activation:** `ConditionActive` and `ConditionCleared` carry `CorrelationId` (key 0x0007),
  the alarm's `RaiseCount`, which is new for each activation, and `ConditionActive` carries
  `Severity` (key 0x0008): High 900, Medium 500, Low 100†, in OPC UA's bands of 1 to 1 000.
- **Operator actions:** a record of an operator's action carries the user as `AckBy` (key
  0x001D), a shelving its seconds as `ShelveSecs` (key 0x001E) and a comment its text as
  `Comment` (key 0x0037).
- A text longer than a slot's 255 bytes (a path, a user, a comment) is cut at a character
  boundary and the record carries the partial-payload flag; `ConditionId` is the hash of the
  whole path.
- Records that do not fit the ring are counted by the ring as usual; the in-memory records of
  §5, item 3, keep every transition regardless.

### 5.1 Requests

Beside the runtime display's requests (specification 39, §12.8):

1. **The list and the records:** `GET /api/hmi/alarms` passes the web authentication with the
   Viewer role and returns `{ "alarms": <list>, "events": [<record>, …] }`:
   - the list of specification 39, §13.6: `active` (unacknowledged first, then by priority, then
     newest first), `shelved`, `suppressed` and `out_of_service`, the counts `unacknowledged` and
     `annunciating`, and `newest`, the alarm that became active last (`null` when none
     annunciates);
   - each alarm has its `path`, `message`, `priority` (`high`, `medium` or `low`), `state`, its
     outputs `active`, `unacknowledged`, `shelved`, `out_of_service`, `suppressed`, `first_out`,
     `annunciating` and `raise_count`, its input `latching`, `since_ms`, when it last became
     active, and `equipment`, the path of the function block instance nearest above it that
     holds it (absent when a program holds it directly; specification 39, §13.6, item 9);
   - the records kept in memory, oldest first. Each has `seq`, `time_ms`, `path`, `kind`
     (`raised`, `cleared`, `acknowledged`, `shelved`, `unshelved`, `suppressed`, `unsuppressed`,
     `out-of-service`, `in-service`, `reset` or `comment`), `message` and `priority`; for an
     operator's action `user`, and for a shelving or a comment `detail` (the seconds or the text);
   - among them, each command's final outcome (specification 39, §12.5, §12.7), page commands and
     alarm actions alike, as a record of kind `command`: `path` is the alarm for an alarm action
     and the target point otherwise, `message` what it asked (for example `Acknowledge` or
     `Write 42`), with `user`, `outcome` (`confirmed`, `applied`, `refused` or `timed-out`) and,
     for a refusal or a time-out, the reason as `detail`.
2. **The value stream** (specification 39, §11.1) sends `{ "alarms": <list> }` when it opens and
   whenever the list changes, and `{ "events": [<record>, …] }` with the records added since its
   last message.
3. **Operator actions:** `POST /api/hmi/alarm` with `{ "id", "alarm", "action", "value"? }`:
   - `alarm` is a path from the alarm list; `action` is `acknowledge`, `shelve` (`value`: seconds,
     1 to 86 400), `unshelve`, `reset`, `comment` (`value`: 1 to 120 characters),
     `remove-from-service` or `return-to-service`;
   - it needs an HMI session with the action's role (§4). The library defines the actions and
     their roles, so no write-policy entry is needed;
   - the answer, the later outcomes on the value stream and the rules for repeated ids are those of
     a command (specification 39, §12.8, item 5);
   - it is confirmed when the instance shows the action applied, within 5 s: `Unacknowledged`
     FALSE, `Shelved` TRUE or FALSE, `Active` FALSE after a reset, `OutOfService` TRUE or FALSE,
     and for a comment the request taken (`Request.Comment` empty again). Acknowledging an alarm
     that waits for no acknowledgement is confirmed at once;
   - the record of the transition names the user; a comment is recorded when it is applied;
   - its audit record has the request type `hmi.alarm`, with the time, the user and session, the
     command id, the alarm, the action, the value, the outcome and the reason.
