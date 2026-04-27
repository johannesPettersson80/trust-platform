# District Pump Network — Proxy + Mediator

A district heating supply pump runs against an aggregated demand from
three customer stations: one local (wired straight into the PLC) and
two remote (delivered over MQTT with quality flags and age timestamps).
The OOP version puts every station behind a `IStationProxy` that hides
"local vs. remote" and "fresh vs. stale", and a single `DemandMediator`
walks all three proxies through the same call site to compute the
aggregated demand and pump command.

## When classic is the right answer

The procedural version is `non-oop/src/Main.st` (80 lines). Use it when:

- Three stations and they will not grow.
- The two staleness rules (quality flag + age) are the only acceptance
  criteria you will ever need.
- Local-vs-remote distinction collapses to "is this signal an MQTT
  topic or a wired tag" once.
- No second mediator coordinates a different group of stations.

The OOP version costs about 3× the lines. It earns that cost when a
fourth station appears, when a third acceptance rule is added (e.g.,
range-check on demand), or when the local station gains its own
quality contract that differs from the remote ones.

## Where classic strains

`ClassicDistrictPumpNetwork.Update` (lines 9-42 of `non-oop/src/Main.st`)
inlines two near-identical `IF QualityGood AND AgeSeconds <= 30 THEN
TotalDemand := TotalDemand + Demand ELSE StaleCount + 1 END_IF` blocks
back-to-back. Adding a fourth station means a third copy of the same
block. Adding a "demand sanity-check" rule (reject if `Demand <
0`) means editing every copy of the block. By the third or fourth
station the `Update` method is the most-edited file in the project.

## Structure

```mermaid
classDiagram
    class IStationProxy {
        <<interface>>
        +StationId : INT
        +DemandM3H : REAL
        +IsStale : BOOL
        +QualityGood : BOOL
        +UpdateProxy(DemandM3H, QualityGoodInput, AgeSeconds)
    }
    IStationProxy <|.. LocalStationProxy
    IStationProxy <|.. RemoteStationProxy

    class DemandMediator {
        +Initialize()
        +Update(LocalDemand, NorthDemand, NorthQuality, NorthAge,
                EastDemand, EastQuality, EastAge, SuctionHealthy) REAL
    }
    DemandMediator *-- LocalStationProxy : Local
    DemandMediator *-- RemoteStationProxy : NorthRemote
    DemandMediator *-- RemoteStationProxy : EastRemote
```

The interface, the two concrete proxies, and `DemandMediator` are all
defined in this example. No OSCAT library FBs are pulled in — the
example is a pure pattern composition.

`LocalStationProxy.IsStale` is hard-coded `FALSE`: a wired tag never
goes stale (or the PLC's own scan would freeze first). `RemoteStationProxy`
encapsulates the staleness rule (`NOT QualityGood OR AgeSeconds > 30`)
inside the proxy itself — the mediator does not know what "stale" means.

## What happens at runtime

```mermaid
sequenceDiagram
    participant Main
    participant M as DemandMediator
    participant L as Local (LocalStationProxy)
    participant N as NorthRemote (RemoteStationProxy)
    participant E as EastRemote (RemoteStationProxy)
    Main->>M: Update(LocalDemand, NorthDemand+Quality+Age, EastDemand+Quality+Age, SuctionHealthy)
    M->>L: UpdateProxy(LocalDemand, TRUE, 0)
    M->>N: UpdateProxy(NorthDemand, NorthQuality, NorthAge)
    M->>E: UpdateProxy(EastDemand, EastQuality, EastAge)
    M->>M: TotalDemand := 0; StaleCount := 0
    M->>L: AcceptProxy(Local, CountStale := FALSE)
    Note over M: Local always counts (never stale)
    M->>N: AcceptProxy(NorthRemote, CountStale := TRUE)
    M->>E: AcceptProxy(EastRemote, CountStale := TRUE)
    alt SuctionHealthy = FALSE
        M-->>Main: PumpSpeed := 0 (Class A trip)
    else
        M-->>Main: PumpSpeed := LIMIT(0, TotalDemand * 2.0, 100)
    end
```

## The keystone

```st
(* Three station updates, one mediator, one accept loop. *)
Local.UpdateProxy(DemandM3H := LocalDemandM3H, QualityGoodInput := TRUE, AgeSeconds := INT#0);
NorthRemote.UpdateProxy(DemandM3H := NorthDemandM3H, QualityGoodInput := NorthQualityGood, AgeSeconds := NorthAgeSeconds);
EastRemote.UpdateProxy(DemandM3H := EastDemandM3H, QualityGoodInput := EastQualityGood, AgeSeconds := EastAgeSeconds);

TotalDemandM3HValue := REAL#0.0;
RemoteStaleCountValue := INT#0;
AcceptProxy(Station := Local, CountStale := FALSE);
AcceptProxy(Station := NorthRemote, CountStale := TRUE);
AcceptProxy(Station := EastRemote, CountStale := TRUE);
```

`AcceptProxy` is one private method that tests `Station.IsStale`,
adds to `TotalDemand` if fresh, increments `StaleCount` if stale and
`CountStale` is set. The `LocalStationProxy.IsStale` always returns
FALSE so the local station is always counted. Adding a fourth station
is one more `Update` call plus one more `AcceptProxy`.

## Patterns used

- [Proxy](../../../docs/guides/oop-concepts-in-st.md#proxy)
- [Mediator](../../../docs/guides/oop-concepts-in-st.md#mediator)

ST mechanics used:

- [Interface](../../../docs/guides/oop-concepts-in-st.md#interface) and
  [IMPLEMENTS](../../../docs/guides/oop-concepts-in-st.md#implements)
- [Polymorphism](../../../docs/guides/oop-concepts-in-st.md#polymorphism)
- [Composition](../../../docs/guides/oop-concepts-in-st.md#composition)

## What this demo doesn't show

- **Persistent last-good fallback.** When a remote goes stale, its
  contribution is dropped. A real district pump might fall back to the
  last-good demand value with an exponential decay. The proxy is the
  right place to put that, but the demo does not implement it.
- **Backpressure / rate limiting.** Demand changes are passed through
  unfiltered. A real install slews the pump command to avoid water
  hammer.
- **Schema validation on the MQTT payload.** `NorthQualityGood` and
  `NorthAgeSeconds` arrive pre-decoded. A real proxy would parse the
  raw payload, range-check it, and report parse errors itself.
- **Per-station alarm classes.** `RemoteStaleCount` is a single counter.
  An ISA-18.2 install would emit a per-station alarm code with priority.
- **Mediator-of-mediators.** A single mediator coordinates three
  stations. A multi-zone district has zone-level mediators feeding
  a plant-level mediator.

## When NOT to use this

- One station, no remotes — the procedural body is shorter.
- Two stations with identical acceptance rules — passing a `BOOL
  CountStale` flag to a small helper FB is shorter than two proxy types.
- All stations are local; staleness is impossible — the proxy abstraction
  buys nothing.

## Integration map

| Tag | Address | Direction |
| --- | --- | --- |
| `Network.LocalDemandRaw` | `%IW0` | IN |
| `Network.NorthDemandRaw` | `%IW2` | IN |
| `Network.EastDemandRaw` | `%IW4` | IN |
| `Network.NorthAgeSeconds` | `%IW6` | IN |
| `Network.EastAgeSeconds` | `%IW8` | IN |
| `Network.NorthQualityGood` | `%IX0.0` | IN |
| `Network.EastQualityGood` | `%IX0.1` | IN |
| `Network.SuctionHealthy` | `%IX0.2` | IN |
| `Network.PumpSpeedRaw` | `%QW0` | OUT |
| `Network.StaleAlarmOut` | `%QX0.0` | OUT |

Comms (from `oop/io.toml`): `mqtt` (broker `127.0.0.1:1883`, topics
`district/pumps/remote/demand` in, `district/pumps/local/snapshot` out),
`modbus-tcp` (slave 61 on `127.0.0.1:1515`).

OPC UA exposed records (from `oop/runtime.toml`, namespace
`urn:trust:examples:district-pump-network-proxy-mediator`):
`Network.TotalDemandM3H`, `Network.PumpSpeedPct`,
`Network.RemoteStaleCount`, `Network.ClassAAlarm`.

## Run

```bash
trust-runtime test --project examples/OSCAT/district_pump_network_proxy_mediator/non-oop
trust-runtime test --project examples/OSCAT/district_pump_network_proxy_mediator/oop
```

---

## Folder Layout

This paired example contains:

- `non-oop/` — the classic Structured Text project.
- `oop/` — the OSCAT OOP Structured Text project.

## What This Example Teaches

OOP pattern: Proxy + Mediator. The OOP version moves decisions behind
named function-block instances and an interface contract; the non-oop
version inlines those decisions in procedural ST.

## How The Pair Teaches OOP

The teaching content above walks through the same machine in both
projects: where classic strains, the structural diagram of the OOP
version, the keystone snippet, and the integration map. Run the pair
side-by-side and read `non-oop/src/Main.st` first.
