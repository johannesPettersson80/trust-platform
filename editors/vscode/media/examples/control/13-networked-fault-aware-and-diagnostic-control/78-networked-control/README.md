# 80. Networked control

You will accept only fresh, newer remote commands, separate network timing from the heater's one-second delivery delay, and transfer to local PI when communication is unavailable.

## The problem

A remote command can be dropped, delayed or reordered while the heater continues operating. A received request is not the same as applied power. Holding an old remote command indefinitely can make a communication fault invisible to the process.

## What we need

We need to prevent old, late or reordered commands from taking control,
limit how long a remote command may be held, and transfer to local control
when communication is unavailable. Recovery must not reapply an obsolete command.

## The new idea

**Networked control** exchanges measurements or commands through a communication
link whose delay, ordering and availability affect operation. An old packet
must not overwrite a newer command, and a lost connection must not leave
remote output applied indefinitely.

With a shared clock, command age is
$a=t_{receiver,now}-t_{sent}$ in seconds. Sequence number identifies which
command is newer within the current connection; connection identity keeps
old-session messages separate; time since the last accepted receipt limits
how long we hold an output. Those are different checks.

Use heater heat capacity 50 kJ/°C and heat loss 10 kW/°C to 20 °C, 2 kW/point, 1 s physical delivery delay, 0–100% command and the fixed 60 s thermal requirements: SP35→38 peak≤39 °C and absolute error≤.2 °C from 30–60 s; separate 30 kW extraction at SP35 minimum≥33 °C and the same error band. Packets have a sequence number and send/receive timestamps from one common simulation clock. A connection **epoch** identifies one explicitly established communication session; it prevents an old session’s messages from being treated as new commands. Within that epoch, accept only newer sequence values with age $0\le age\le.2$ s. The receiver holds the last accepted command for at most .3 s from its last accepted receipt, then uses local valid-PV PI fallback. Network time and physical delivery time are separate.

In this simulation, sender and receiver use one clock. On equipment, send-time age would additionally require a known clock offset and uncertainty; unsynchronized timestamps cannot establish it. A packet is accepted when its sequence is newer than the last accepted sequence, timestamps are monotonic within the epoch, and age is within .2 s. A drop or reordering therefore cannot overwrite a newer command. Commands are sent every .1 s and the receiver checks its timeout every .1 s. A single dropped packet at 5.0 s is normally followed by a valid packet at 5.1 s, so the last command is held briefly and fallback need not occur. Choose a precise boundary: fallback starts when elapsed time is greater than .3 s.

During remote mode, the local PI state tracks applied output. On remote-to-local fallback, use $I=u_a-P$, skip ordinary integration on the transfer scan, and resume normal integration on later scans. Local fallback requires healthy PV; invalid PV uses Lesson 28 zero heat. Returning to remote requires operator acknowledgement, three fresh newer packets and confirmation that remote PI has tracked applied output. An old held command cannot automatically restart control.

## Prediction

Predict that one dropped packet holds the command until the next accepted packet, a packet with sequence 50 arriving after 51 is rejected, and an age .25 s packet is rejected. With last receipt at 4.9 s and no packets from 5.0 through 5.9 s, the hold is .3 s at 5.2 and .4 s at 5.3. Predict fallback at 5.3 s. Predict why a one-second applied-power delay cannot be inferred from packet age.

## Implementation

Write the packet acceptance rule with retained sequence, epoch and last-receipt time. Initialize `HaveAcceptedPacket : BOOL := FALSE;` for each confirmed new epoch. It permits sequence zero as the first packet; after acceptance, later packets must have a larger sequence. Use `UDINT` for the sequence and epoch: it stores nonnegative whole numbers up to 4,294,967,295. Establish a new epoch before the sequence wraps; do not treat wrapped old values as newer. `PacketValid` below includes valid finite timestamps, valid finite command and acceptable timestamp order. The normal temperature quality checks remain separate.

```st

    IF PacketValid
        AND PacketEpoch = CurrentEpoch
        AND (NOT HaveAcceptedPacket OR PacketSequence > LastSequence)
        AND PacketAgeS >= 0.0
        AND PacketAgeS <= 0.2
        AND PacketCommandPct >= 0.0
        AND PacketCommandPct <= 100.0 THEN
        HaveAcceptedPacket := TRUE;
        LastSequence := PacketSequence;
        LastReceiptS := ReceiverTimeS;
        RemoteCommandPct := PacketCommandPct;
        RemoteValid := TRUE;
    END_IF;
```

If receiver time minus LastReceiptS is greater than .3 s, clear remote authority and select local fallback. Expose send, receive, age, sequence, accepted/applied command and physical delayed command. A confirmed epoch change resets `HaveAcceptedPacket` to FALSE, clears remote authority and starts a new acknowledgement/fresh-packet sequence; it must not immediately apply the old stored command.

## Experiment in Control Studio

1. Use the selected heater PI gains in both remote and local instances, with beta=1, filter bypass, .1 s updates and tracking time 1 s. Reset PV 35 °C, SP 35 °C, applied/remote command 75%, local integral 75%, $D=0$, full one-second applied-command history 75%, acknowledged epoch, first valid sequence 0 at t=0, common clock and valid PV. Display all packet and physical timing fields.
2. Run normal 60 s SP35→38 and separate 60 s 30 kW tests. For each of those two thermal tests, run separate 60 s variants with one packet dropped at 5.0 s; packet 51 received at 5.1 s before delayed packet 50 at 5.2 s; and a packet sent at 5.0 but received at 5.25 s. Keep all other packets on time. The latter packet fails both age and sequence checks if a newer packet has arrived. Record rejected packets, held output and temperature performance.
3. In another full-reset 60 s variant for each thermal event, block packets from 5.0 through 5.9 s. Require local fallback at 5.3 s and a transfer-induced command jump no greater than .1 point. At 6 s acknowledge recovery; count new accepted packets at 6.0, 6.1 and 6.2 s. At 6.4 s, permit a separate remote-enable edge only if the remote controller has aligned its request with current applied output within .1 point. Otherwise remain in local PI. Subsequent integral updates proceed normally; do not count one packet repeatedly as three.
4. Stop for PV outside 0–80 °C, applied command outside 0–100%, invalid clocks or unresponsive output. On normal end cancel events, command zero heat, restore and fully reset before each case.

## Evidence and decision

Normal and fault variants require separate closed-loop records; do not replay one whole PV trace after changing packet events. Check sequence/age acceptance, hold duration, local transfer continuity and three-packet recovery. The network fault path does not command zero heat while PV is healthy; invalid PV follows Lesson 28.

## Conclusion

Networked heater control needs a monotonic epoch/sequence contract, bounded packet age, a finite hold interval, and local PI fallback with output tracking. Packet timing and one-second heater delivery are separate evidence channels.

## Reason for the next lesson

Even with fresh network data, two heater sensors can disagree. Fault-aware control must distinguish a detectable disagreement from knowledge of which sensor is correct.

[Fault-aware redundant sensing](../79-fault-aware-control/README.md) handles that case.
