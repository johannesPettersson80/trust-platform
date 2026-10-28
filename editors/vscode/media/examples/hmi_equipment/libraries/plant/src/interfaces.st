(* The capabilities of the Plant blocks (specification trust-plant-library §5). Each interface is
   small; a block implements the ones it supports. Operator commands return TRUE when accepted.
   An HMI button may call only a method of an interface its block implements. *)
NAMESPACE Plant

(* Mode and reset, common to every device (§3). *)
INTERFACE IDevice
    METHOD SetMode : BOOL
    VAR_INPUT
        Value : E_Mode;
    END_VAR
    END_METHOD
    METHOD GetMode : E_Mode
    END_METHOD
    METHOD Reset : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IRunDevice EXTENDS IDevice
    METHOD Start : BOOL
    END_METHOD
    METHOD Stop : BOOL
    END_METHOD
    METHOD IsRunning : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ISpeedDevice EXTENDS IRunDevice
    METHOD SetSpeed : BOOL
    VAR_INPUT
        Value : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IPowerDevice EXTENDS IRunDevice
    METHOD SetPower : BOOL
    VAR_INPUT
        Value : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IReversingDevice EXTENDS IDevice
    METHOD StartForward : BOOL
    END_METHOD
    METHOD StartReverse : BOOL
    END_METHOD
    METHOD Stop : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ITwoSpeedDevice EXTENDS IDevice
    METHOD StartSlow : BOOL
    END_METHOD
    METHOD StartFast : BOOL
    END_METHOD
    METHOD Stop : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IOpenCloseDevice EXTENDS IDevice
    METHOD Open : BOOL
    END_METHOD
    METHOD Close : BOOL
    END_METHOD
    METHOD IsOpen : BOOL
    END_METHOD
    METHOD IsClosed : BOOL
    END_METHOD
END_INTERFACE

(* A motor-operated valve can stop between its end positions. *)
INTERFACE ITravelStop
    METHOD StopTravel : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IExtendRetractDevice EXTENDS IDevice
    METHOD Extend : BOOL
    END_METHOD
    METHOD Retract : BOOL
    END_METHOD
    METHOD IsExtended : BOOL
    END_METHOD
    METHOD IsRetracted : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IPositionDevice EXTENDS IDevice
    METHOD SetPosition : BOOL
    VAR_INPUT
        Value : LREAL;
    END_VAR
    END_METHOD
    METHOD GetPosition : LREAL
    END_METHOD
END_INTERFACE

(* An Engineer confirms that the service is done (§4.7). *)
INTERFACE IServiceable
    METHOD ServiceDone : BOOL
    END_METHOD
END_INTERFACE

(* A run device a group can command in Auto (§9.2). *)
INTERFACE IDutyMember EXTENDS IRunDevice
    METHOD IsAvailable : BOOL
    END_METHOD
    METHOD Demand
    VAR_INPUT
        Run : BOOL;
    END_VAR
    END_METHOD
    METHOD GetRunHours : LREAL
    END_METHOD
END_INTERFACE

INTERFACE IMeasurement
    METHOD GetValue : LREAL
    END_METHOD
    METHOD GetQuality : E_Quality
    END_METHOD
END_INTERFACE

INTERFACE ITotal
    METHOD ResetTotal : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ICounter
    METHOD ResetCount : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ITare
    METHOD Tare : BOOL
    END_METHOD
    METHOD ClearTare : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ISetpoint
    METHOD SetSetpoint : BOOL
    VAR_INPUT
        Value : LREAL;
    END_VAR
    END_METHOD
END_INTERFACE

(* An Engineer bypasses one condition of a set for a limited time (§9.1). *)
INTERFACE IBypass
    METHOD Bypass : BOOL
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
    METHOD RemoveBypass : BOOL
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* The alarms a block holds, for an alarm manager (§4.5, §9.3). *)
INTERFACE IAlarmSource
    METHOD GetAlarmCount : INT
    END_METHOD
    METHOD GetAlarm : REF_TO Alarm.Alarm
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IAlarmManager
    METHOD AcknowledgeAll : BOOL
    END_METHOD
    METHOD Silence : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ISequence
    METHOD Start : BOOL
    END_METHOD
    METHOD Stop : BOOL
    END_METHOD
    METHOD Hold : BOOL
    END_METHOD
    METHOD Resume : BOOL
    END_METHOD
    METHOD Abort : BOOL
    END_METHOD
    METHOD Reset : BOOL
    END_METHOD
    METHOD GetState : E_SequenceState
    END_METHOD
END_INTERFACE

INTERFACE ISequenceManager
    METHOD StopAll : BOOL
    END_METHOD
    METHOD AbortAll : BOOL
    END_METHOD
END_INTERFACE

END_NAMESPACE
