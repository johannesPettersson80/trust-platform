(* The operator panel's texts, built in the slow panel task from the values the control task gives
   the panel: the state of the batch with its live values, the most important alarm, and a report
   line for the last batch. *)

(* The number of a state in the panel's register map. *)
FUNCTION StateCode : INT
VAR_INPUT
    State : VatState;
END_VAR
CASE State OF
    VatState#Idle: StateCode := 0;
    VatState#Filling: StateCode := 1;
    VatState#Heating: StateCode := 2;
    VatState#Holding: StateCode := 3;
    VatState#Cooling: StateCode := 4;
    VatState#Draining: StateCode := 5;
    VatState#Done: StateCode := 6;
    VatState#Stopped: StateCode := 7;
    VatState#Aborted: StateCode := 8;
END_CASE;
END_FUNCTION

(* A value with one decimal, as a panel shows it: 63.2. *)
FUNCTION OneDecimal : STRING[12]
VAR_INPUT
    Value : REAL;
END_VAR
VAR
    Tenths : DINT;
END_VAR
Tenths := REAL_TO_DINT(ABS(Value) * 10.0);
OneDecimal := CONCAT(DINT_TO_STRING(Tenths / 10), '.', DINT_TO_STRING(Tenths MOD 10));
IF Value < 0.0 THEN
    OneDecimal := CONCAT('-', OneDecimal);
END_IF;
END_FUNCTION

(* A duration as minutes and seconds: 17:05. *)
FUNCTION MinutesSeconds : STRING[12]
VAR_INPUT
    Duration : TIME;
END_VAR
VAR
    Seconds : UDINT;
END_VAR
Seconds := DWORD_TO_UDINT(TIME_TO_DWORD(Duration)) / 1000;
IF Seconds MOD 60 < 10 THEN
    MinutesSeconds := CONCAT(UDINT_TO_STRING(Seconds / 60), ':0', UDINT_TO_STRING(Seconds MOD 60));
ELSE
    MinutesSeconds := CONCAT(UDINT_TO_STRING(Seconds / 60), ':', UDINT_TO_STRING(Seconds MOD 60));
END_IF;
END_FUNCTION

PROGRAM PanelTexts
VAR_EXTERNAL
    Status : VatStatus;
END_VAR
VAR
    LastBatches : UDINT;
    (* In the order of the alarm conditions of VatControl. A variable, since the compiler cannot
       yet index a constant array by a variable. *)
    AlarmTexts : ARRAY[1..12] OF STRING[28] := [
        'Emergency stop',
        'Level high-high',
        'Level transmitter fault',
        'Temperature sensor fault',
        'Over-temperature',
        'Agitator fault',
        'Inlet valve fault',
        'Outlet valve fault',
        'Cooling valve fault',
        'Drain pump fault',
        'Heating takes too long',
        'Hold restarted: too cool'
    ];
END_VAR

CASE Status.State OF
    VatState#Idle:
        Status.StateText := CONCAT('Ready, recipe ', Status.Recipe);
    VatState#Filling:
        Status.StateText := CONCAT('Filling ', OneDecimal(Value := Status.Level), ' %');
    VatState#Heating:
        Status.StateText := CONCAT('Heating ', OneDecimal(Value := Status.Temperature), ' C to ',
            OneDecimal(Value := Status.Setpoint), ' C');
    VatState#Holding:
        Status.StateText := CONCAT('Holding ', OneDecimal(Value := Status.Temperature), ' C, ',
            MinutesSeconds(Duration := Status.HoldRemaining), ' left');
    VatState#Cooling:
        Status.StateText := CONCAT('Cooling ', OneDecimal(Value := Status.Temperature), ' C');
    VatState#Draining:
        Status.StateText := CONCAT('Draining ', OneDecimal(Value := Status.Level), ' %');
    VatState#Done:
        Status.StateText := 'Batch complete';
    VatState#Stopped:
        Status.StateText := CONCAT('Stopped for ', MinutesSeconds(Duration := Status.StepTime),
            ', Start resumes');
    VatState#Aborted:
        Status.StateText := 'Aborted: clear the alarms, then Reset';
END_CASE;

IF Status.FirstOut > 0 THEN
    Status.AlarmText := AlarmTexts[Status.FirstOut];
ELSE
    Status.AlarmText := '';
END_IF;

IF Status.Batches <> LastBatches THEN
    Status.BatchReport := CONCAT('Batch ', UDINT_TO_STRING(Status.Batches), ': ', Status.Recipe,
        ' pasteurized at ', OneDecimal(Value := Status.Setpoint), ' C');
    LastBatches := Status.Batches;
END_IF;
END_PROGRAM
