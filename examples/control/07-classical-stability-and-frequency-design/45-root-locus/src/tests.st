(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. Control Studio calculates the root locus. *)

TEST_PROGRAM TheLawOfBothShafts
(* u = Kp·(r − θ) − 7·ω: at rest 1 rad short of the target, the first torque equals Kp: 12, 16 or 20 N·m. *)
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := BrakedPositionTorque(ProportionalGain := LREAL#12.0, VelocityGain := LREAL#7.0,
    Target := LREAL#1.0, Angle := LREAL#0.0, Velocity := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#16.0, ACTUAL := BrakedPositionTorque(ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0,
    Target := LREAL#1.0, Angle := LREAL#0.0, Velocity := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := BrakedPositionTorque(ProportionalGain := LREAL#20.0, VelocityGain := LREAL#7.0,
    Target := LREAL#1.0, Angle := LREAL#0.0, Velocity := LREAL#0.0), DELTA := LREAL#1.0e-12);
(* Halfway, moving at 0.5 rad/s: 16·0.5 − 7·0.5 = 4.5 N·m; the braking term opposes the motion. *)
ASSERT_NEAR(EXPECTED := LREAL#4.5, ACTUAL := BrakedPositionTorque(ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0,
    Target := LREAL#1.0, Angle := LREAL#0.5, Velocity := LREAL#0.5), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
