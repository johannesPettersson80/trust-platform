(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM TheFirstPacketOfAConnectionMayHaveSequenceZero
(* A new connection starts from the command already applied, with remote authority; its first packet may carry sequence 0. *)
VAR
    Receiver : CommandReceiver;
    Packet : CommandPacket;
END_VAR
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
ASSERT_TRUE(IN := Receiver.RemoteAuthority);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#0;
Packet.CommandPct := LREAL#77.55;
Receiver(NowMs := UDINT#100, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#75.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.LastSequence);
ASSERT_NEAR(EXPECTED := LREAL#77.55, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.HoldMs);
ASSERT_EQUAL(EXPECTED := UDINT#100, ACTUAL := Receiver.AgeMs);
END_TEST_PROGRAM

TEST_PROGRAM OnlyNewerPacketsOfTheCurrentConnectionAreAccepted
(* A repeated or older sequence number is not newer; a packet of connection 6 is refused whatever its sequence number. A new
   connection, 8, starts its sequence again at 0, and packets of connection 7 are then old-session packets. *)
VAR
    Receiver : CommandReceiver;
    Packet : CommandPacket;
END_VAR
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#0;
Packet.CommandPct := LREAL#76.0;
Receiver(NowMs := UDINT#100, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#75.0, Acknowledge := FALSE);
Packet.SentMs := UDINT#100;
Packet.CommandPct := LREAL#90.0;
Receiver(NowMs := UDINT#200, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.RejectedNotNewer);
ASSERT_NEAR(EXPECTED := LREAL#76.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
Packet.Sequence := UDINT#2;
Packet.SentMs := UDINT#200;
Packet.CommandPct := LREAL#78.0;
Receiver(NowMs := UDINT#300, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#2, ACTUAL := Receiver.LastSequence);
Packet.Sequence := UDINT#1;
Packet.SentMs := UDINT#300;
Packet.CommandPct := LREAL#77.0;
Receiver(NowMs := UDINT#400, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#78.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#2, ACTUAL := Receiver.RejectedNotNewer);
ASSERT_NEAR(EXPECTED := LREAL#78.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
Packet.Epoch := UDINT#6;
Packet.Sequence := UDINT#4000;
Packet.SentMs := UDINT#450;
Packet.CommandPct := LREAL#0.0;
Receiver(NowMs := UDINT#500, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#78.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.RejectedOldSession);
ASSERT_NEAR(EXPECTED := LREAL#78.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
Receiver.Establish(NewEpoch := UDINT#8, AtMs := UDINT#600, CommandPct := LREAL#78.0);
Packet.Epoch := UDINT#8;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#600;
Packet.CommandPct := LREAL#79.0;
Receiver(NowMs := UDINT#700, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#78.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.LastSequence);
ASSERT_NEAR(EXPECTED := LREAL#79.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#50;
Packet.SentMs := UDINT#700;
Packet.CommandPct := LREAL#10.0;
Receiver(NowMs := UDINT#800, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#79.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.RejectedOldSession);
ASSERT_NEAR(EXPECTED := LREAL#79.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheAgeMustBeWithin0To200Milliseconds
(* Age is receive time minus send time on the shared clock: 200 ms is accepted, 300 ms is refused, and a packet sent after
   it was received (negative age) is refused as well. *)
VAR
    Receiver : CommandReceiver;
    Packet : CommandPacket;
END_VAR
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#0;
Packet.CommandPct := LREAL#76.0;
Receiver(NowMs := UDINT#200, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#75.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.LastSequence);
ASSERT_EQUAL(EXPECTED := UDINT#200, ACTUAL := Receiver.AgeMs);
Packet.Sequence := UDINT#1;
Packet.SentMs := UDINT#100;
Packet.CommandPct := LREAL#77.0;
Receiver(NowMs := UDINT#400, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.RejectedByAge);
ASSERT_EQUAL(EXPECTED := UDINT#300, ACTUAL := Receiver.AgeMs);
Packet.Sequence := UDINT#2;
Packet.SentMs := UDINT#600;
Receiver(NowMs := UDINT#500, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#2, ACTUAL := Receiver.RejectedByAge);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.LastSequence);
ASSERT_NEAR(EXPECTED := LREAL#76.0, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheLastCommandIsHeld300MillisecondsThenControlIsLocal
(* After the packet accepted at 100 ms nothing arrives: the command is held for 100, 200 and 300 ms, and at 400 ms, more than
   300 ms after the last accepted packet, remote authority ends. Packets that arrive afterwards are accepted, but without an
   acknowledgement control stays local. *)
VAR
    Receiver : CommandReceiver;
    Packet : CommandPacket;
    T : UDINT;
END_VAR
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#0;
Packet.CommandPct := LREAL#76.0;
Receiver(NowMs := UDINT#100, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#75.0, Acknowledge := FALSE);
FOR T := 200 TO 400 BY 100 DO
    Receiver(NowMs := T, Arrived := FALSE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
    ASSERT_EQUAL(EXPECTED := T - UDINT#100, ACTUAL := Receiver.HoldMs);
    ASSERT_TRUE(IN := Receiver.RemoteAuthority);
END_FOR;
Receiver(NowMs := UDINT#500, Arrived := FALSE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#400, ACTUAL := Receiver.HoldMs);
ASSERT_FALSE(IN := Receiver.RemoteAuthority);
FOR T := 600 TO 900 BY 100 DO
    Packet.Sequence := T / UDINT#100;
    Packet.SentMs := T - UDINT#100;
    Receiver(NowMs := T, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
    ASSERT_EQUAL(EXPECTED := Packet.Sequence, ACTUAL := Receiver.LastSequence);
    ASSERT_FALSE(IN := Receiver.RemoteAuthority);
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM RemoteControlReturnsAfterAcknowledgementThreeFreshPacketsAndAlignment
(* After a fallback, the acknowledgement at 800 ms starts counting accepted packets. A repeated packet is not newer and does not
   count. The third fresh packet commands 85 % while 80 % is applied: not aligned, control stays local. The fourth commands
   80.05 %, within 0.1 point: remote control returns with that fresh command, never with the command held before the outage. *)
VAR
    Receiver : CommandReceiver;
    Packet : CommandPacket;
END_VAR
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
Packet.Epoch := UDINT#7;
Packet.Sequence := UDINT#0;
Packet.SentMs := UDINT#0;
Packet.CommandPct := LREAL#76.0;
Receiver(NowMs := UDINT#100, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#75.0, Acknowledge := FALSE);
Receiver(NowMs := UDINT#500, Arrived := FALSE, Packet := Packet, AppliedPct := LREAL#76.0, Acknowledge := FALSE);
ASSERT_FALSE(IN := Receiver.RemoteAuthority);
Packet.Sequence := UDINT#6;
Packet.SentMs := UDINT#600;
Packet.CommandPct := LREAL#80.0;
Receiver(NowMs := UDINT#700, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Receiver.Fresh);
Packet.Sequence := UDINT#7;
Packet.SentMs := UDINT#700;
Packet.CommandPct := LREAL#80.05;
Receiver(NowMs := UDINT#800, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := TRUE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.Fresh);
Receiver(NowMs := UDINT#900, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.Fresh);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Receiver.RejectedNotNewer);
Packet.Sequence := UDINT#9;
Packet.SentMs := UDINT#900;
Packet.CommandPct := LREAL#80.0;
Receiver(NowMs := UDINT#1000, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#2, ACTUAL := Receiver.Fresh);
Packet.Sequence := UDINT#10;
Packet.SentMs := UDINT#1000;
Packet.CommandPct := LREAL#85.0;
Receiver(NowMs := UDINT#1100, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#3, ACTUAL := Receiver.Fresh);
ASSERT_FALSE(IN := Receiver.RemoteAuthority);
Packet.Sequence := UDINT#11;
Packet.SentMs := UDINT#1100;
Packet.CommandPct := LREAL#80.05;
Receiver(NowMs := UDINT#1200, Arrived := TRUE, Packet := Packet, AppliedPct := LREAL#80.0, Acknowledge := FALSE);
ASSERT_TRUE(IN := Receiver.RemoteAuthority);
ASSERT_NEAR(EXPECTED := LREAL#80.05, ACTUAL := Receiver.RemoteCommandPct, DELTA := LREAL#0.0);
END_TEST_PROGRAM
