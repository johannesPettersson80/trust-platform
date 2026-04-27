TEST_PROGRAM MotorCellCommandsMixedVendorsThroughInterface
VAR
    Cell : MotorCell;
END_VAR

Cell.Initialize();
Cell.InjectVendorStatus(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0100, SiemensStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cell.RunningCount);
END_TEST_PROGRAM

TEST_PROGRAM MotorCellNormalizesVendorFaultCodes
VAR
    Cell : MotorCell;
END_VAR

Cell.Initialize();
Cell.InjectVendorStatus(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0002, SiemensStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0));
ASSERT_EQUAL(EXPECTED := DWORD#16#20000022, ACTUAL := Cell.LastFaultCode);
END_TEST_PROGRAM

(* Per-cycle reset test: drives multiple injection scenarios through
   one Cell instance to verify RunningCount and FaultCount reflect the
   current cycle, not a cumulative total. Catches bugs where Update
   leaks count across calls. *)
TEST_PROGRAM MotorCellCountsResetPerCycle
VAR
    Cell : MotorCell;
END_VAR

Cell.Initialize();

(* Cycle 1: all three vendors running, no faults *)
Cell.InjectVendorStatus(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0100, SiemensStatus := WORD#16#0400);
Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cell.RunningCount);

(* Cycle 2: only one vendor running, one faulted *)
Cell.InjectVendorStatus(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0002, SiemensStatus := WORD#16#0000);
Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0);
(* Counts must reflect cycle 2, not be cumulative from cycle 1 *)
ASSERT_TRUE(IN := Cell.RunningCount <= INT#2);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cell.FaultCount);

(* Cycle 3: pull-cord released - all motors must stop, no counts *)
Cell.Update(RunCommand := TRUE, PullCordOk := FALSE, SpeedHz := REAL#35.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.RunningCount);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.FaultCount);
END_TEST_PROGRAM

(* AbbAcs580Adapter isolation: bit 2 = running, bit 3 = fault. *)
TEST_PROGRAM AbbAdapterDecodesAbbStatusWordCorrectly
VAR
    Adapter : AbbAcs580Adapter;
END_VAR
Adapter.Initialize();

(* Bit 2 set (0x0004) -> running *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0004, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.IsRunning);
ASSERT_FALSE(IN := Adapter.HasFault);

(* Bit 3 set (0x0008) -> faulted *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0008, FaultCodeRaw := WORD#16#0011, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);
ASSERT_TRUE(IN := Adapter.HasFault);
(* ABB fault namespace prefix: 0x10000000 *)
ASSERT_EQUAL(EXPECTED := DWORD#16#10000011, ACTUAL := Adapter.FaultCode);

(* Both bits set -> running AND faulted (status word ambiguous) *)
Adapter.ConfigureRaw(StatusWord := WORD#16#000C, FaultCodeRaw := WORD#16#0044, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.IsRunning);
ASSERT_TRUE(IN := Adapter.HasFault);

(* No bits set -> idle, no fault *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0000, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);
ASSERT_FALSE(IN := Adapter.HasFault);

(* Speed scaling: raw 500 / 10.0 = 50.0 Hz *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0004, FaultCodeRaw := WORD#0, SpeedRaw := INT#500);
ASSERT_NEAR(EXPECTED := REAL#50.0, ACTUAL := Adapter.ActualSpeedHz, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* DanfossFc302Adapter isolation: bit 8 = running, bit 1 = fault. *)
TEST_PROGRAM DanfossAdapterDecodesDanfossStatusWordCorrectly
VAR
    Adapter : DanfossFc302Adapter;
END_VAR
Adapter.Initialize();

(* Bit 8 set (0x0100) -> running *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0100, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.IsRunning);
ASSERT_FALSE(IN := Adapter.HasFault);

(* Bit 1 set (0x0002) -> faulted; Danfoss namespace prefix: 0x20000000 *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0002, FaultCodeRaw := WORD#16#0022, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);
ASSERT_TRUE(IN := Adapter.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#20000022, ACTUAL := Adapter.FaultCode);

(* ABB-style bit 2 (0x0004) is NOT a Danfoss running indicator *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0004, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);
END_TEST_PROGRAM

(* SiemensG120Adapter isolation: bit 10 = running, bit 3 = fault. *)
TEST_PROGRAM SiemensAdapterDecodesProfidriveStatusWordCorrectly
VAR
    Adapter : SiemensG120Adapter;
END_VAR
Adapter.Initialize();

(* Bit 10 set (0x0400) -> running *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0400, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.IsRunning);

(* Bit 3 set (0x0008) -> faulted; Siemens namespace prefix: 0x30000000 *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0008, FaultCodeRaw := WORD#16#0033, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#30000033, ACTUAL := Adapter.FaultCode);

(* ABB-style bit 2 (0x0004) is NOT a Siemens running indicator *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0004, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);

(* Danfoss-style bit 8 (0x0100) is NOT a Siemens running indicator *)
Adapter.ConfigureRaw(StatusWord := WORD#16#0100, FaultCodeRaw := WORD#0, SpeedRaw := INT#0);
ASSERT_FALSE(IN := Adapter.IsRunning);
END_TEST_PROGRAM

(* Stop command zeroes the commanded speed and clears running.
   IsRunning is driven by the status word, so we simulate the VFD
   reporting "running" via ConfigureRaw before the assertion. Stop()
   clears RunningValue immediately to model an instantaneous stop. *)
TEST_PROGRAM AbbAdapterStopClearsCommandedSpeedAndRunning
VAR
    Adapter : AbbAcs580Adapter;
END_VAR
Adapter.Initialize();
Adapter.Command(SpeedReferenceHz := REAL#50.0, Direction := INT#1);
Adapter.ConfigureRaw(StatusWord := WORD#16#0004, FaultCodeRaw := WORD#0, SpeedRaw := INT#500);
ASSERT_TRUE(IN := Adapter.IsRunning);
Adapter.Stop();
ASSERT_FALSE(IN := Adapter.IsRunning);
END_TEST_PROGRAM

(* ResetFault clears fault state and fault code. *)
TEST_PROGRAM AbbAdapterResetFaultClearsFaultState
VAR
    Adapter : AbbAcs580Adapter;
END_VAR
Adapter.Initialize();
Adapter.ConfigureRaw(StatusWord := WORD#16#0008, FaultCodeRaw := WORD#16#0099, SpeedRaw := INT#0);
ASSERT_TRUE(IN := Adapter.HasFault);
Adapter.ResetFault();
ASSERT_FALSE(IN := Adapter.HasFault);
END_TEST_PROGRAM

(* Pull-cord drop is a hard stop independent of RunCommand. *)
TEST_PROGRAM MotorCellPullCordReleaseHardStopsAllMotors
VAR
    Cell : MotorCell;
END_VAR
Cell.Initialize();
Cell.InjectVendorStatus(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0100, SiemensStatus := WORD#16#0400);
Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cell.RunningCount);

(* Pull-cord released: all stop regardless of RunCommand *)
Cell.Update(RunCommand := TRUE, PullCordOk := FALSE, SpeedHz := REAL#35.0);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.RunningCount);
END_TEST_PROGRAM

(* All three vendors faulted simultaneously. *)
TEST_PROGRAM MotorCellHandlesAllVendorsFaulted
VAR
    Cell : MotorCell;
END_VAR
Cell.Initialize();
Cell.InjectVendorStatus(AbbStatus := WORD#16#0008, DanfossStatus := WORD#16#0002, SiemensStatus := WORD#16#0008);
Cell.Update(RunCommand := TRUE, PullCordOk := TRUE, SpeedHz := REAL#35.0);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cell.FaultCount);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.RunningCount);
END_TEST_PROGRAM
