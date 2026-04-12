TEST_PROGRAM plcopen_motion_synchronization_master_slave_phase_relationships_are_deterministic
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    Seed : TRUST_SYNC_TestSeed;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
END_VAR

Master.AxisId := UDINT#1;
Master.InternalIndex := UINT#1;
Slave.AxisId := UDINT#2;
Slave.InternalIndex := UINT#2;
Seed(
    Master := Master,
    Slave := Slave,
    StartPosition := REAL#10.0,
    ExitPosition := REAL#40.0,
    RatioNumerator := REAL#2.0,
    RatioDenominator := REAL#1.0,
    PhaseOffset := REAL#1.5,
    LoadDefaultCam := TRUE
);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#5.0, NextActualMasterPosition := REAL#5.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_FALSE(Advance.InSync);
ASSERT_TRUE(Advance.GearSlavePosition = REAL#11.5);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_TRUE(Advance.InSync);
ASSERT_TRUE(Advance.GearSlavePosition = REAL#21.5);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#12.0, NextActualMasterPosition := REAL#12.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_TRUE(Advance.InSync);
ASSERT_TRUE(Advance.GearSlavePosition = REAL#25.5);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#41.0, NextActualMasterPosition := REAL#41.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_FALSE(Advance.InSync);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_cam_activation_timing_is_deterministic
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    Seed : TRUST_SYNC_TestSeed;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
END_VAR

Master.AxisId := UDINT#3;
Master.InternalIndex := UINT#3;
Slave.AxisId := UDINT#4;
Slave.InternalIndex := UINT#4;
Seed(
    Master := Master,
    Slave := Slave,
    StartPosition := REAL#10.0,
    ExitPosition := REAL#50.0,
    RatioNumerator := REAL#1.0,
    RatioDenominator := REAL#1.0,
    PhaseOffset := REAL#0.0,
    LoadDefaultCam := TRUE
);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#9.0, NextActualMasterPosition := REAL#9.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_FALSE(Advance.InSync);
ASSERT_TRUE(Advance.CamSlavePosition = REAL#0.0);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_TRUE(Advance.InSync);
ASSERT_TRUE(Advance.CamSlavePosition = REAL#5.0);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#25.0, NextActualMasterPosition := REAL#25.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
ASSERT_TRUE(Advance.InSync);
ASSERT_TRUE(Advance.CamSlavePosition = REAL#15.0);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#35.0, NextActualMasterPosition := REAL#35.0, UseActualMasterPosition := FALSE, BreakSync := TRUE);
ASSERT_FALSE(Advance.InSync);
ASSERT_TRUE(Advance.CamSlavePosition = REAL#30.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_public_surface_resolves
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    CamIn : MC_CamIn;
    CamOut : MC_CamOut;
    GearIn : MC_GearIn;
    GearOut : MC_GearOut;
    GearInPos : MC_GearInPos;
END_VAR

Master.AxisId := UDINT#5;
Master.InternalIndex := UINT#5;
Slave.AxisId := UDINT#6;
Slave.InternalIndex := UINT#6;
CamTable.CamId := UINT#6;
CamTable.NumberOfPairs := UINT#1;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.SlavePosition0 := REAL#0.0;
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := FALSE, Periodic := FALSE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
CamIn(Master := Master, Slave := Slave, Execute := FALSE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#0.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#0, BufferMode := MC_BUFFER_MODE#mcAborting);
CamOut(Slave := Slave, Execute := FALSE);
GearIn(Master := Master, Slave := Slave, Execute := FALSE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
GearOut(Slave := Slave, Execute := FALSE);
GearInPos(Master := Master, Slave := Slave, Execute := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#0.0, SlaveSyncPosition := REAL#0.0, SyncMode := MC_SYNC_MODE#mcShortest, MasterStartDistance := REAL#0.0, Velocity := REAL#1.0, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);

ASSERT_FALSE(SelectCam.Done);
ASSERT_FALSE(CamIn.Active);
ASSERT_FALSE(CamOut.Done);
ASSERT_FALSE(GearIn.Active);
ASSERT_FALSE(GearOut.Done);
ASSERT_FALSE(GearInPos.Active);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_sync_types_resolve
VAR
    StartAbsolute : MC_START_MODE;
    StartRelative : MC_START_MODE;
    StartRampIn : MC_START_MODE;
    SyncShortest : MC_SYNC_MODE;
    SyncCatchUp : MC_SYNC_MODE;
    SyncSlowDown : MC_SYNC_MODE;
END_VAR

StartAbsolute := MC_START_MODE#mcAbsolute;
StartRelative := MC_START_MODE#mcRelative;
StartRampIn := MC_START_MODE#mcRampIn;
SyncShortest := MC_SYNC_MODE#mcShortest;
SyncCatchUp := MC_SYNC_MODE#mcCatchUp;
SyncSlowDown := MC_SYNC_MODE#mcSlowDown;

ASSERT_TRUE(StartAbsolute <> StartRelative);
ASSERT_TRUE(StartRampIn = MC_START_MODE#mcRampIn);
ASSERT_TRUE(SyncShortest <> SyncCatchUp);
ASSERT_TRUE(SyncSlowDown = MC_SYNC_MODE#mcSlowDown);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_cam_table_select_prepares_camtableid_and_rejects_delayed
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    Probe : TRUST_SYNC_TestProbe;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    DelayedSelect : MC_CamTableSelect;
END_VAR

Master.AxisId := UDINT#7;
Master.InternalIndex := UINT#7;
Slave.AxisId := UDINT#8;
Slave.InternalIndex := UINT#8;
CamTable.CamId := UINT#11;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;

SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(SelectCam.Busy);
ASSERT_FALSE(SelectCam.Done);
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_FALSE(SelectCam.Busy);
ASSERT_TRUE(SelectCam.Done);
ASSERT_TRUE(SelectCam.CamTableID = UINT#11);
Probe(Axis := Slave);
ASSERT_TRUE(Probe.SelectedCamReady);
ASSERT_TRUE(Probe.SelectedCamID = UINT#11);

DelayedSelect(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(DelayedSelect.Error);
ASSERT_TRUE(DelayedSelect.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_camin_absolute_relative_and_actual_source_are_distinct
VAR
    MasterAbs : AXIS_REF;
    SlaveAbs : AXIS_REF;
    MasterRel : AXIS_REF;
    SlaveRel : AXIS_REF;
    MasterActual : AXIS_REF;
    SlaveActual : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectAbs : MC_CamTableSelect;
    SelectRel : MC_CamTableSelect;
    SelectActual : MC_CamTableSelect;
    CamInAbs : MC_CamIn;
    CamInRel : MC_CamIn;
    CamInActual : MC_CamIn;
    AdvanceAbs : TRUST_SYNC_AdvanceMasterAxis;
    AdvanceRel : TRUST_SYNC_AdvanceMasterAxis;
    AdvanceActual : TRUST_SYNC_AdvanceMasterAxis;
    ProbeAbs : TRUST_SYNC_TestProbe;
    ProbeRel : TRUST_SYNC_TestProbe;
    ProbeActual : TRUST_SYNC_TestProbe;
END_VAR

CamTable.CamId := UINT#21;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;

MasterAbs.AxisId := UDINT#9;
MasterAbs.InternalIndex := UINT#9;
SlaveAbs.AxisId := UDINT#10;
SlaveAbs.InternalIndex := UINT#10;
SelectAbs(Master := MasterAbs, Slave := SlaveAbs, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectAbs(Master := MasterAbs, Slave := SlaveAbs, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
CamInAbs(Master := MasterAbs, Slave := SlaveAbs, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
AdvanceAbs(Master := MasterAbs, Slave := SlaveAbs, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamInAbs(Master := MasterAbs, Slave := SlaveAbs, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
ProbeAbs(Axis := SlaveAbs);
ASSERT_TRUE(CamInAbs.InSync);
ASSERT_TRUE(ProbeAbs.State = MC_AXIS_STATUS#mcSynchronizedMotion);
ASSERT_TRUE(ProbeAbs.CommandedPosition = REAL#5.0);

MasterRel.AxisId := UDINT#11;
MasterRel.InternalIndex := UINT#11;
SlaveRel.AxisId := UDINT#12;
SlaveRel.InternalIndex := UINT#12;
SelectRel(Master := MasterRel, Slave := SlaveRel, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectRel(Master := MasterRel, Slave := SlaveRel, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
AdvanceRel(Master := MasterRel, Slave := SlaveRel, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamInRel(Master := MasterRel, Slave := SlaveRel, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#20.0, StartMode := MC_START_MODE#mcRelative, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
AdvanceRel(Master := MasterRel, Slave := SlaveRel, NextCommandedMasterPosition := REAL#20.0, NextActualMasterPosition := REAL#20.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamInRel(Master := MasterRel, Slave := SlaveRel, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#20.0, StartMode := MC_START_MODE#mcRelative, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
ProbeRel(Axis := SlaveRel);
ASSERT_TRUE(CamInRel.InSync);
ASSERT_TRUE(ProbeRel.CommandedPosition = REAL#5.0);

MasterActual.AxisId := UDINT#13;
MasterActual.InternalIndex := UINT#13;
SlaveActual.AxisId := UDINT#14;
SlaveActual.InternalIndex := UINT#14;
SelectActual(Master := MasterActual, Slave := SlaveActual, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectActual(Master := MasterActual, Slave := SlaveActual, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
CamInActual(Master := MasterActual, Slave := SlaveActual, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcActualValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
AdvanceActual(Master := MasterActual, Slave := SlaveActual, NextCommandedMasterPosition := REAL#20.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := TRUE, BreakSync := FALSE);
CamInActual(Master := MasterActual, Slave := SlaveActual, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcActualValue, CamTableID := UINT#21, BufferMode := MC_BUFFER_MODE#mcAborting);
ProbeActual(Axis := SlaveActual);
ASSERT_TRUE(CamInActual.InSync);
ASSERT_TRUE(ProbeActual.CommandedPosition = REAL#5.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_camout_leaves_synchronized_motion_without_standstill
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    CamIn : MC_CamIn;
    CamOut : MC_CamOut;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
    Probe : TRUST_SYNC_TestProbe;
END_VAR

Master.AxisId := UDINT#15;
Master.InternalIndex := UINT#15;
Slave.AxisId := UDINT#16;
Slave.InternalIndex := UINT#16;
CamTable.CamId := UINT#31;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#31, BufferMode := MC_BUFFER_MODE#mcAborting);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#31, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(CamIn.InSync);
CamOut(Slave := Slave, Execute := TRUE);
ASSERT_TRUE(CamOut.Done);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#31, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(CamIn.CommandAborted);
Probe(Axis := Slave);
ASSERT_TRUE(Probe.State = MC_AXIS_STATUS#mcContinuousMotion);
ASSERT_FALSE(Probe.State = MC_AXIS_STATUS#mcStandstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_camin_missing_selected_cam_errors
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamIn : MC_CamIn;
END_VAR

Master.AxisId := UDINT#17;
Master.InternalIndex := UINT#17;
Slave.AxisId := UDINT#18;
Slave.InternalIndex := UINT#18;
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#1, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(CamIn.Error);
ASSERT_TRUE(CamIn.ErrorID = WORD#16#0102);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_gearin_tracks_master_ratio_and_ingear
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    GearIn : MC_GearIn;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
    Probe : TRUST_SYNC_TestProbe;
END_VAR

Master.AxisId := UDINT#19;
Master.InternalIndex := UINT#19;
Slave.AxisId := UDINT#20;
Slave.InternalIndex := UINT#20;
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#2, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#5.0, NextActualMasterPosition := REAL#5.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#2, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
Probe(Axis := Slave);
ASSERT_TRUE(GearIn.InGear);
ASSERT_TRUE(Probe.State = MC_AXIS_STATUS#mcSynchronizedMotion);
ASSERT_TRUE(Probe.CommandedPosition = REAL#10.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_gearinpos_startsync_then_insync
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    GearInPos : MC_GearInPos;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
    Probe : TRUST_SYNC_TestProbe;
END_VAR

Master.AxisId := UDINT#21;
Master.InternalIndex := UINT#21;
Slave.AxisId := UDINT#22;
Slave.InternalIndex := UINT#22;
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcCatchUp, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#14.0, NextActualMasterPosition := REAL#14.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcCatchUp, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_FALSE(GearInPos.StartSync);
ASSERT_FALSE(GearInPos.InSync);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#15.0, NextActualMasterPosition := REAL#15.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcCatchUp, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(GearInPos.StartSync);
ASSERT_FALSE(GearInPos.InSync);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#20.0, NextActualMasterPosition := REAL#20.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcCatchUp, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
Probe(Axis := Slave);
ASSERT_TRUE(GearInPos.InSync);
ASSERT_TRUE(Probe.CommandedPosition = REAL#100.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_gearout_requires_synchronized_motion
VAR
    Slave : AXIS_REF;
    GearOut : MC_GearOut;
END_VAR

Slave.AxisId := UDINT#23;
Slave.InternalIndex := UINT#23;
GearOut(Slave := Slave, Execute := TRUE);
ASSERT_TRUE(GearOut.Error);
ASSERT_TRUE(GearOut.ErrorID = WORD#16#0200);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_buffered_command_only_becomes_active_on_promotion
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    GearIn : MC_GearIn;
    CamIn : MC_CamIn;
    GearOut : MC_GearOut;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
    Probe : TRUST_SYNC_TestProbe;
END_VAR

Master.AxisId := UDINT#24;
Master.InternalIndex := UINT#24;
Slave.AxisId := UDINT#25;
Slave.InternalIndex := UINT#25;
CamTable.CamId := UINT#41;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#41, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GearIn.Active);
ASSERT_TRUE(CamIn.Busy);
ASSERT_FALSE(CamIn.Active);
Probe(Axis := Slave);
ASSERT_TRUE(Probe.HasQueuedCommand);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#5.0, NextActualMasterPosition := REAL#5.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearOut(Slave := Slave, Execute := TRUE);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#41, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(CamIn.Active);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_aborting_command_flushes_active_and_queued_sync_commands
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    GearIn : MC_GearIn;
    GearInPos : MC_GearInPos;
    CamIn : MC_CamIn;
END_VAR

Master.AxisId := UDINT#26;
Master.InternalIndex := UINT#26;
Slave.AxisId := UDINT#27;
Slave.InternalIndex := UINT#27;
CamTable.CamId := UINT#51;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcShortest, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#51, BufferMode := MC_BUFFER_MODE#mcAborting);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
GearInPos(Master := Master, Slave := Slave, Execute := TRUE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, MasterSyncPosition := REAL#20.0, SlaveSyncPosition := REAL#100.0, SyncMode := MC_SYNC_MODE#mcShortest, MasterStartDistance := REAL#5.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GearIn.CommandAborted);
ASSERT_TRUE(GearInPos.CommandAborted);
ASSERT_TRUE(CamIn.Active);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_synchronization_cam_and_gear_end_to_end_scenario
VAR
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
    SelectCam : MC_CamTableSelect;
    GearIn : MC_GearIn;
    GearOut : MC_GearOut;
    CamIn : MC_CamIn;
    Advance : TRUST_SYNC_AdvanceMasterAxis;
    Probe : TRUST_SYNC_TestProbe;
END_VAR

Master.AxisId := UDINT#28;
Master.InternalIndex := UINT#28;
Slave.AxisId := UDINT#29;
Slave.InternalIndex := UINT#29;
CamTable.CamId := UINT#61;
CamTable.NumberOfPairs := UINT#4;
CamTable.IsAbsolute := TRUE;
CamTable.MasterPosition0 := REAL#0.0;
CamTable.MasterPosition1 := REAL#10.0;
CamTable.MasterPosition2 := REAL#20.0;
CamTable.MasterPosition3 := REAL#30.0;
CamTable.SlavePosition0 := REAL#0.0;
CamTable.SlavePosition1 := REAL#5.0;
CamTable.SlavePosition2 := REAL#15.0;
CamTable.SlavePosition3 := REAL#30.0;
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SelectCam(Master := Master, Slave := Slave, CamTable := CamTable, Execute := TRUE, Periodic := TRUE, MasterAbsolute := TRUE, SlaveAbsolute := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#61, BufferMode := MC_BUFFER_MODE#mcBuffered);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#5.0, NextActualMasterPosition := REAL#5.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
GearIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, RatioNumerator := INT#1, RatioDenominator := UINT#1, MasterValueSource := MC_SOURCE#mcSetValue, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GearIn.InGear);
GearOut(Slave := Slave, Execute := TRUE);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#61, BufferMode := MC_BUFFER_MODE#mcBuffered);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#10.0, NextActualMasterPosition := REAL#10.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#61, BufferMode := MC_BUFFER_MODE#mcBuffered);
Advance(Master := Master, Slave := Slave, NextCommandedMasterPosition := REAL#30.0, NextActualMasterPosition := REAL#30.0, UseActualMasterPosition := FALSE, BreakSync := FALSE);
CamIn(Master := Master, Slave := Slave, Execute := TRUE, ContinuousUpdate := FALSE, MasterOffset := REAL#0.0, SlaveOffset := REAL#0.0, MasterScaling := REAL#1.0, SlaveScaling := REAL#1.0, MasterStartDistance := REAL#0.0, MasterSyncPosition := REAL#10.0, StartMode := MC_START_MODE#mcAbsolute, MasterValueSource := MC_SOURCE#mcSetValue, CamTableID := UINT#61, BufferMode := MC_BUFFER_MODE#mcBuffered);
Probe(Axis := Slave);
ASSERT_TRUE(CamIn.InSync);
ASSERT_TRUE(CamIn.EndOfProfile);
ASSERT_TRUE(Probe.State = MC_AXIS_STATUS#mcSynchronizedMotion);
ASSERT_TRUE(Probe.CommandedPosition = REAL#30.0);
END_TEST_PROGRAM
