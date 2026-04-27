TEST_PROGRAM SeparatorUsesAdapterAndStateObjects
VAR
    Separator : SeparatorController;
END_VAR

Separator.Initialize();
Separator.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#0.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0044);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Separator.BowlRunning);

Separator.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0044);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Separator.FeedValveOpen);
END_TEST_PROGRAM

TEST_PROGRAM SeparatorAdapterNormalizesVibrationFault
VAR
    Separator : SeparatorController;
END_VAR

Separator.Initialize();
Separator.InjectDriveStatus(UseSiemens := TRUE, StatusWord := WORD#16#0400, SpeedFeedbackRpm := REAL#6100.0, VibrationMmSRaw := REAL#9.5, FaultRaw := WORD#16#0012);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#16#500000F1, ACTUAL := Separator.NormalizedFaultCode);
END_TEST_PROGRAM

(* Vendor-swap test: drives same separator with both ABB and Siemens
   adapter status formats. Catches bugs where the adapter selection
   sticks to the first vendor seen, or where decoded values from the
   wrong vendor leak into the application after a swap. *)
TEST_PROGRAM SeparatorAdapterSwapBetweenVendorsOnSameInstance
VAR
    Separator : SeparatorController;
END_VAR

Separator.Initialize();

(* ABB vendor (UseSiemens := FALSE), bowl spun up *)
Separator.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0044);
Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE);
ASSERT_TRUE(IN := Separator.BowlRunning);

(* Siemens vendor (UseSiemens := TRUE) on the same controller *)
Separator.InjectDriveStatus(UseSiemens := TRUE, StatusWord := WORD#16#0400, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0011);
Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := TRUE);
ASSERT_TRUE(IN := Separator.BowlRunning);
END_TEST_PROGRAM

(* Per-adapter isolation: ABB drive alone, no controller. Bit 0x0004
   running, bit 0x0008 fault, prefix 0xD000_____. *)
TEST_PROGRAM SeparatorAbbAdapterDecodesIsolated
VAR
    Abb : AbbSeparatorDriveAdapter;
END_VAR

(* Idle: nothing set *)
Abb.Decode(StatusWord := WORD#16#0000, SpeedFeedbackRpm := REAL#0.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Abb.IsRunning);
ASSERT_FALSE(IN := Abb.HasFault);

(* Running and at-speed: bit 0x0004 + speed >= 6000 *)
Abb.Decode(StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0044);
ASSERT_TRUE(IN := Abb.IsRunning);
ASSERT_TRUE(IN := Abb.IsAtSpeed);

(* Fault bit set *)
Abb.Decode(StatusWord := WORD#16#0008, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0044);
ASSERT_TRUE(IN := Abb.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#D0000044, ACTUAL := Abb.FaultCode);

(* Vibration trip overrides FaultCode regardless of FaultRaw *)
Abb.Decode(StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#10.0, FaultRaw := WORD#16#0044);
ASSERT_TRUE(IN := Abb.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#D00000F1, ACTUAL := Abb.FaultCode);
END_TEST_PROGRAM

(* Per-adapter isolation: Siemens drive. Bit 0x0400 running, 0x0008 fault,
   prefix 0x5000_____. Confirms ABB bit (0x0004) does NOT register. *)
TEST_PROGRAM SeparatorSiemensAdapterDecodesIsolated
VAR
    Sie : SiemensSeparatorDriveAdapter;
END_VAR

(* ABB running bit on Siemens adapter: not running *)
Sie.Decode(StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0011);
ASSERT_FALSE(IN := Sie.IsRunning);

(* Siemens running bit *)
Sie.Decode(StatusWord := WORD#16#0400, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0011);
ASSERT_TRUE(IN := Sie.IsRunning);
ASSERT_TRUE(IN := Sie.IsAtSpeed);

(* Vibration trip with Siemens namespace *)
Sie.Decode(StatusWord := WORD#16#0400, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#9.0, FaultRaw := WORD#16#0011);
ASSERT_EQUAL(EXPECTED := DWORD#16#500000F1, ACTUAL := Sie.FaultCode);

(* Standard fault with Siemens namespace, no vibration *)
Sie.Decode(StatusWord := WORD#16#0008, SpeedFeedbackRpm := REAL#6000.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0011);
ASSERT_TRUE(IN := Sie.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#50000011, ACTUAL := Sie.FaultCode);
END_TEST_PROGRAM

(* IsAtSpeed boundary: exactly 6000 RPM is the threshold (>= comparison). *)
TEST_PROGRAM SeparatorAtSpeedBoundary
VAR
    Abb : AbbSeparatorDriveAdapter;
END_VAR

(* Just below 6000 *)
Abb.Decode(StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#5999.9, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Abb.IsAtSpeed);

(* Exactly 6000 *)
Abb.Decode(StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6000.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0000);
ASSERT_TRUE(IN := Abb.IsAtSpeed);

(* Above with fault: not at speed *)
Abb.Decode(StatusWord := WORD#16#0008, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#2.0, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Abb.IsAtSpeed);
END_TEST_PROGRAM

(* Vibration trip boundary: > 8.0 mm/s strictly. *)
TEST_PROGRAM SeparatorVibrationBoundary
VAR
    Sep : SeparatorController;
END_VAR

(* At 8.0 exact: no trip *)
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#8.0, FaultRaw := WORD#16#0000);
ASSERT_NOT_EQUAL(EXPECTED := INT#90, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));

(* Above 8.0: trip *)
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#8.1, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
END_TEST_PROGRAM

(* CIP transition: from any non-fault state, CIP mode forces 40 (CIP).
   When CIP is dropped, returns to Idle (not the prior state). *)
TEST_PROGRAM SeparatorCipModeFromMultipleStates
VAR
    Sep : SeparatorController;
END_VAR

(* Idle -> CIP *)
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0000, SpeedFeedbackRpm := REAL#0.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Sep.Update(StartCommand := FALSE, DischargeRequest := FALSE, CipMode := TRUE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Sep.CipValveOpen);
ASSERT_FALSE(IN := Sep.FeedValveOpen);

(* CIP -> Idle when CIP off *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Sep.Update(StartCommand := FALSE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_FALSE(IN := Sep.CipValveOpen);
END_TEST_PROGRAM

(* SpinUp -> CIP transition skips Production. *)
TEST_PROGRAM SeparatorSpinUpInterruptedByCip
VAR
    Sep : SeparatorController;
END_VAR
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#3000.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
(* CIP request mid-spin-up *)
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := TRUE, UseSiemens := FALSE));
END_TEST_PROGRAM

(* Production -> Discharge -> Production cycle. Discharge state forwards
   back to Production when DischargeRequest drops. *)
TEST_PROGRAM SeparatorDischargeCycleReturnsToProduction
VAR
    Sep : SeparatorController;
END_VAR
Sep.Initialize();
(* Spin up to at-speed *)
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
(* AtSpeed -> Production *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Sep.FeedValveOpen);
(* DischargeRequest -> Discharge *)
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := TRUE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Sep.DischargeValveOpen);
ASSERT_FALSE(IN := Sep.FeedValveOpen);
(* Drop request -> back to Production *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
ASSERT_TRUE(IN := Sep.FeedValveOpen);
ASSERT_FALSE(IN := Sep.DischargeValveOpen);
END_TEST_PROGRAM

(* Production state demands AtSpeed; if speed drops mid-production,
   the controller must fault. *)
TEST_PROGRAM SeparatorProductionFaultsOnSpeedDropout
VAR
    Sep : SeparatorController;
END_VAR
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));

(* Speed drops below 6000: no AtSpeed *)
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#5500.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
END_TEST_PROGRAM

(* Fault state is terminal. *)
TEST_PROGRAM SeparatorFaultStateIsTerminal
VAR
    Sep : SeparatorController;
END_VAR
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0008, SpeedFeedbackRpm := REAL#0.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0033);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Sep.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
(* Even with all healthy inputs *)
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmSRaw := REAL#1.0, FaultRaw := WORD#16#0000);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Sep.Update(StartCommand := FALSE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE));
END_TEST_PROGRAM

(* Empty inputs hold idle state. *)
TEST_PROGRAM SeparatorEmptyInputsHoldIdle
VAR
    Sep : SeparatorController;
    I : INT;
END_VAR
Sep.Initialize();
Sep.InjectDriveStatus(UseSiemens := FALSE, StatusWord := WORD#16#0000, SpeedFeedbackRpm := REAL#0.0, VibrationMmSRaw := REAL#0.0, FaultRaw := WORD#16#0000);
FOR I := 1 TO 5 DO
    Sep.Update(StartCommand := FALSE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE);
END_FOR;
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Sep.StateId);
ASSERT_FALSE(IN := Sep.FeedValveOpen);
ASSERT_FALSE(IN := Sep.DischargeValveOpen);
ASSERT_FALSE(IN := Sep.CipValveOpen);
END_TEST_PROGRAM
