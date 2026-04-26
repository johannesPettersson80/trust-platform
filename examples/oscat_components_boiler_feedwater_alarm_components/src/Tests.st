TEST_PROGRAM BoilerFeedwaterAlarmComponentsQueueAlarmCode
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmQueue : DwordFifo16;
    QueueObject : IDwordQueue;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
AlarmObject := LevelAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
QueueObject := AlarmQueue;
QueueObject.Initialize();
QueueObject.Reset();

ASSERT_TRUE(IN := AlarmObject.Update(MeasuredValue := REAL#12.0));
ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#B001));
ASSERT_TRUE(IN := QueueObject.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := QueueObject.Value);
END_TEST_PROGRAM
