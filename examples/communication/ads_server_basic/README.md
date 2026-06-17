# Communication Example: ADS Server

This example exposes truST runtime globals as Beckhoff ADS symbols so an
external ADS client can browse, read, subscribe, and optionally write a guarded
test variable.

## What you learn

- how `[runtime.ads_server]` exposes declared globals as ADS symbols
- why the ADS server uses TCP `48898` while the logical ADS port remains `851`
- how source-IP-pinned clients are allowlisted
- how pyads proof differs from the real TwinCAT merge gate
- how writes stay disabled by default and require explicit writable symbols

## Files in this folder

- `src/config.st`: globals exposed as `global.TankLevel`, `global.PumpRunning`,
  `global.Setpoint`, and `global.StatusWord`
- `src/main.st`: simple logic that keeps values changing through the normal scan
- `runtime.toml`: ADS server config for local pyads smoke testing
- `io.toml`, `trust-lsp.toml`: simulated I/O and project defaults

## TwinCAT side checklist

Use this checklist when TwinCAT is the ADS client and truST is the ADS server:

1. Start the truST runtime with `[runtime.ads_server]` enabled.
2. In TwinCAT XAE, open `SYSTEM > Routes`.
3. Use `Add > Broadcast Search` when UDP `48899` is allowed, select the truST
   runtime, and add the route. truST acknowledges that Add Route handshake so
   TwinCAT can finish its local route setup. Actual ADS access is still denied
   unless the TwinCAT client is listed in `[runtime.ads_server.clients]`.
   If broadcast is blocked, use the generated route artifact from Step 4.
4. Browse the truST target on logical ADS port `851`.
5. Read symbols with their full wire names, for example `global.TankLevel` and
   `global.Setpoint`.
6. Test writes only on symbols that truST explicitly lists in `writable`.

The route belongs on the TwinCAT/client side in server mode. This is the
opposite direction from the ADS client import example.

## Step 1: Build the project

```bash
cargo build -p trust-runtime --features ads-server
cargo run -p trust-runtime --features ads-server --bin trust-runtime -- \
  build --project examples/communication/ads_server_basic --sources src
```

## Step 2: Start the runtime

Run in one terminal:

```bash
cargo run -p trust-runtime --features ads-server --bin trust-runtime -- \
  play --project examples/communication/ads_server_basic --no-console
```

The example binds ADS/TCP on `127.0.0.1:48898`, UDP identify on
`127.0.0.1:48899`, and serves logical ADS port `851` under AMS Net ID
`127.0.0.1.1.1`.

## Step 3: Run the pyads smoke test

In another terminal, install pyads in a local venv and run:

```bash
python3 -m venv .venv-pyads
.venv-pyads/bin/python -m pip install pyads
.venv-pyads/bin/python scripts/ads_server_pyads_smoke.py \
  --target-ip 127.0.0.1 \
  --target-net-id 127.0.0.1.1.1 \
  --local-net-id 127.0.0.1.1.100 \
  --read global.TankLevel:REAL \
  --read global.PumpRunning:BOOL \
  --read global.StatusWord:WORD \
  --notification global.PumpRunning:BOOL \
  --write global.Setpoint:REAL:12.5 \
  --doctor-endpoint unix:///tmp/trust-runtime-ads-server-basic.sock \
  --doctor-token ads-server-smoke-token \
  --trust-runtime target/debug/trust-runtime
```

The script performs:

- device info and ADS state read
- symbol browse
- handle resolve and read by handle
- read by name
- sum-up list read
- `ADSIGRP_SYM_VERSION` read
- notification subscription
- guarded write and restore for `global.Setpoint`
- optional `ads.server.doctor` call with `external_kind = "pyads"`

The JSON report includes `"twinCAT_merge_gate_satisfied": false`. pyads is an
independent client proof, but it is not the real TwinCAT engineering-station
merge gate.

## Step 4: Generate a TwinCAT route artifact

For a real engineering station, generate the route artifact from the truST ADS
server identity:

```bash
cargo run -p trust-runtime --features ads-server --bin trust-runtime -- \
  ads server route-script \
    --route-name trust-runtime-ads-server-basic \
    --server-ip 192.168.77.10 \
    --server-net-id 192.168.77.10.1.1 \
    --format powershell
```

Run that artifact on the ADS client side, not on the truST runtime host. In
server mode, TwinCAT/pyads/.NET is the client and needs a route to the truST ADS
target.

## Expected Doctor output

Without an external client proof, the Doctor should pass the loopback self-test
but remain short of production-ready:

```bash
cargo run -p trust-runtime --features ads-server --bin trust-runtime -- \
  ads server doctor --project examples/communication/ads_server_basic
```

After the pyads smoke script completes, attach that evidence:

```bash
cargo run -p trust-runtime --features ads-server --bin trust-runtime -- \
  ads server doctor \
    --project examples/communication/ads_server_basic \
    --external-kind pyads \
    --external-name pyads-smoke
```

The expected JSON report has `overall = "pass"`,
`external_client_verified = true`, and `external_client_kind = "pyads"`. That
still does not complete the real TwinCAT merge gate.

## TwinCAT validation

For the production gate, use TwinCAT from a Windows engineering station:

1. Browse for the truST ADS target and confirm it appears in Broadcast Search.
2. Add a route from the TwinCAT machine to the truST runtime host using that
   broadcast result. If broadcast is blocked, use the server route artifact from
   `trust-runtime ads server route-script`.
   The Add Route acknowledgement is a setup compatibility response; it does not
   add a client to truST's allowlist.
3. Browse symbols and confirm datatype upload works.
4. Read `global.TankLevel`, `global.PumpRunning`, and `global.StatusWord`.
5. Write only a guarded test variable such as `global.Setpoint`, then confirm
   the runtime audit logs an `ads.server.write` event.

For a quick live-value check in TwinCAT XAE, open the Target Browser, select
`truST-ADS -> 851 -> global.Setpoint`, and use the Target Browser value preview.
Scope View is not required for ADS server validation; it adds a separate Scope
Server recording workflow and should be treated as optional interop, not the
primary proof path.

For a notification proof from the Windows engineering station, use Beckhoff's
ADS.NET client and make the test change the value itself. This avoids the
common false failure where the subscription is running but the PLC-side mirror
variable was edited without pulsing the ADS write command.

```powershell
$work = "$env:TEMP\trust-ads-notify"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $work | Out-Null
Set-Location $work
dotnet new console --framework net8.0
dotnet add package Beckhoff.TwinCAT.Ads
```

Replace `Program.cs` with:

```csharp
using System;
using System.Threading;
using TwinCAT.Ads;

class Program
{
    static void Main()
    {
        const string netId = "192.168.77.10.1.1";
        const int port = 851;
        const string symbol = "global.Setpoint";

        using var changed = new ManualResetEventSlim(false);
        using var client = new AdsClient();

        client.Connect(netId, port);
        var valueHandle = client.CreateVariableHandle(symbol);
        var notificationHandle = 0u;
        var expected = 0.0f;

        try
        {
            var initial = Convert.ToSingle(client.ReadAny(valueHandle, typeof(float)));
            expected = initial + 1.25f;
            Console.WriteLine($"Initial {symbol} = {initial}");

            client.AdsNotificationEx += (_sender, e) =>
            {
                var value = Convert.ToSingle(e.Value);
                Console.WriteLine($"NOTIFICATION {symbol} = {value}");
                if (Math.Abs(value - expected) < 0.001f)
                {
                    changed.Set();
                }
            };

            var settings = new NotificationSettings(AdsTransMode.OnChange, 100, 0);
            notificationHandle = client.AddDeviceNotificationEx(
                symbol,
                settings,
                null,
                typeof(float));

            Thread.Sleep(500);
            Console.WriteLine($"Writing {symbol} = {expected}");
            client.WriteAny(valueHandle, expected);

            if (!changed.Wait(TimeSpan.FromSeconds(10)))
            {
                throw new TimeoutException("No changed notification arrived within 10 seconds.");
            }

            Console.WriteLine("Notification changed-value proof passed.");
        }
        finally
        {
            if (notificationHandle != 0)
            {
                client.DeleteDeviceNotification(notificationHandle);
            }
            client.DeleteVariableHandle(valueHandle);
        }
    }
}
```

Then run:

```powershell
dotnet run
```

Expected output includes the initial value, one notification with the initial
value, the write, and a second notification with the changed value.
Beckhoff ADS.NET sends notification cycle and max-delay values on the ADS wire
in 100 ns units; truST normalizes those values before scheduling samples, so a
`cycleTime` of `100` in this script produces ordinary 100 ms notification
polling rather than a minutes-long delay.

### TwinCAT PLC smoke program

Add the TwinCAT PLC library `Tc2_DataExchange`. Beckhoff documents
`FB_ReadAdsSymByName` and `FB_WriteAdsSymByName` there; both are edge-triggered,
so the command input must pulse instead of staying true forever. For the
optional sum-up proof below, also add `Tc2_System` for `ADSRDWRT`.

Create a GVL named `GVL_truST_Test`:

```iecst
VAR_GLOBAL
    TargetNetId : T_AmsNetId := '192.168.77.10.1.1';
    TargetPort  : T_AmsPort := 851;

    CmdReadAll       : BOOL := FALSE;
    CmdWriteSetpoint : BOOL := FALSE;
    CmdSumRead       : BOOL := FALSE;
    CmdSumReadWrite  : BOOL := FALSE;

    TankLevel        : REAL;
    PumpRunning      : BOOL;
    StatusWord       : WORD;
    SetpointReadback : REAL;
    SetpointWrite    : REAL := 55.5;
    SumSetpoint      : REAL;
    SumTankLevel     : REAL;
    SumResult0       : UDINT;
    SumResult1       : UDINT;
    SumReadOk        : BOOL;
    SumRwSetpoint    : REAL;
    SumRwTankLevel   : REAL;
    SumRwResult0     : UDINT;
    SumRwLength0     : UDINT;
    SumRwResult1     : UDINT;
    SumRwLength1     : UDINT;
    SumReadWriteOk   : BOOL;

    Busy             : BOOL;
    AnyError         : BOOL;
    LastErrorId      : UDINT;
END_VAR
```

For sum-up read, add these DUT declarations:

```iecst
TYPE ST_truST_SumReadItem :
STRUCT
    IndexGroup  : UDINT;
    IndexOffset : UDINT;
    Length      : UDINT;
END_STRUCT
END_TYPE

TYPE ST_truST_SumReadRequest2 :
STRUCT
    Item0 : ST_truST_SumReadItem;
    Item1 : ST_truST_SumReadItem;
END_STRUCT
END_TYPE

TYPE ST_truST_SumReadResponse2 :
STRUCT
    Result0  : UDINT;
    Result1  : UDINT;
    Setpoint : REAL;
    TankLevel: REAL;
END_STRUCT
END_TYPE

TYPE ST_truST_SumReadWriteItem :
STRUCT
    IndexGroup  : UDINT;
    IndexOffset : UDINT;
    ReadLength  : UDINT;
    WriteLength : UDINT;
END_STRUCT
END_TYPE

TYPE ST_truST_SumReadWriteRequest2 :
STRUCT
    Item0 : ST_truST_SumReadWriteItem;
    Item1 : ST_truST_SumReadWriteItem;
END_STRUCT
END_TYPE

TYPE ST_truST_SumReadWriteResponse2 :
STRUCT
    Result0  : UDINT;
    Length0  : UDINT;
    Result1  : UDINT;
    Length1  : UDINT;
    Setpoint : REAL;
    TankLevel: REAL;
END_STRUCT
END_TYPE
```

Use this `MAIN` program for the first read/write proof:

```iecst
PROGRAM MAIN
VAR
    fbReadTank      : FB_ReadAdsSymByName;
    fbReadPump      : FB_ReadAdsSymByName;
    fbReadStatus    : FB_ReadAdsSymByName;
    fbReadSetpoint  : FB_ReadAdsSymByName;
    fbWriteSetpoint : FB_WriteAdsSymByName;
    fbSumRead       : ADSRDWRT;
    fbSumReadWrite  : ADSRDWRT;
    sumReq          : ST_truST_SumReadRequest2;
    sumResp         : ST_truST_SumReadResponse2;
    sumRwReq        : ST_truST_SumReadWriteRequest2;
    sumRwResp       : ST_truST_SumReadWriteResponse2;
    step            : UINT := 0;
END_VAR

GVL_truST_Test.Busy := step <> 0;

CASE step OF
0:
    fbReadTank(bRead := FALSE);
    fbReadPump(bRead := FALSE);
    fbReadStatus(bRead := FALSE);
    fbReadSetpoint(bRead := FALSE);
    fbWriteSetpoint(bWrite := FALSE);
    fbSumRead(WRTRD := FALSE);
    fbSumReadWrite(WRTRD := FALSE);

    IF GVL_truST_Test.CmdReadAll THEN
        GVL_truST_Test.AnyError := FALSE;
        GVL_truST_Test.LastErrorId := 0;
        GVL_truST_Test.CmdReadAll := FALSE;
        step := 10;
    ELSIF GVL_truST_Test.CmdWriteSetpoint THEN
        GVL_truST_Test.AnyError := FALSE;
        GVL_truST_Test.LastErrorId := 0;
        GVL_truST_Test.CmdWriteSetpoint := FALSE;
        step := 100;
    ELSIF GVL_truST_Test.CmdSumRead THEN
        GVL_truST_Test.AnyError := FALSE;
        GVL_truST_Test.LastErrorId := 0;
        GVL_truST_Test.SumReadOk := FALSE;
        GVL_truST_Test.CmdSumRead := FALSE;

        // truST assigns the example symbols to index group 16#4020:
        // global.Setpoint at offset 1, global.TankLevel at offset 7.
        sumReq.Item0.IndexGroup := 16#4020;
        sumReq.Item0.IndexOffset := 1;
        sumReq.Item0.Length := SIZEOF(GVL_truST_Test.SumSetpoint);
        sumReq.Item1.IndexGroup := 16#4020;
        sumReq.Item1.IndexOffset := 7;
        sumReq.Item1.Length := SIZEOF(GVL_truST_Test.SumTankLevel);
        step := 200;
    ELSIF GVL_truST_Test.CmdSumReadWrite THEN
        GVL_truST_Test.AnyError := FALSE;
        GVL_truST_Test.LastErrorId := 0;
        GVL_truST_Test.SumReadWriteOk := FALSE;
        GVL_truST_Test.CmdSumReadWrite := FALSE;

        // Same two reads, but through ADSIGRP_SUMUP_READWRITE (16#F082).
        // WriteLength is zero here: this validates the response layout without
        // changing the writable Setpoint.
        sumRwReq.Item0.IndexGroup := 16#4020;
        sumRwReq.Item0.IndexOffset := 1;
        sumRwReq.Item0.ReadLength := SIZEOF(GVL_truST_Test.SumRwSetpoint);
        sumRwReq.Item0.WriteLength := 0;
        sumRwReq.Item1.IndexGroup := 16#4020;
        sumRwReq.Item1.IndexOffset := 7;
        sumRwReq.Item1.ReadLength := SIZEOF(GVL_truST_Test.SumRwTankLevel);
        sumRwReq.Item1.WriteLength := 0;
        step := 300;
    END_IF

10:
    fbReadTank(
        bRead := TRUE,
        sNetId := GVL_truST_Test.TargetNetId,
        nPort := GVL_truST_Test.TargetPort,
        sVarName := 'global.TankLevel',
        nDestAddr := ADR(GVL_truST_Test.TankLevel),
        nLen := SIZEOF(GVL_truST_Test.TankLevel),
        tTimeout := T#2S);
    IF NOT fbReadTank.bBusy THEN
        IF fbReadTank.bError THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbReadTank.nErrorId;
            step := 0;
        ELSE
            step := 20;
        END_IF
    END_IF

20:
    fbReadPump(
        bRead := TRUE,
        sNetId := GVL_truST_Test.TargetNetId,
        nPort := GVL_truST_Test.TargetPort,
        sVarName := 'global.PumpRunning',
        nDestAddr := ADR(GVL_truST_Test.PumpRunning),
        nLen := SIZEOF(GVL_truST_Test.PumpRunning),
        tTimeout := T#2S);
    IF NOT fbReadPump.bBusy THEN
        IF fbReadPump.bError THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbReadPump.nErrorId;
            step := 0;
        ELSE
            step := 30;
        END_IF
    END_IF

30:
    fbReadStatus(
        bRead := TRUE,
        sNetId := GVL_truST_Test.TargetNetId,
        nPort := GVL_truST_Test.TargetPort,
        sVarName := 'global.StatusWord',
        nDestAddr := ADR(GVL_truST_Test.StatusWord),
        nLen := SIZEOF(GVL_truST_Test.StatusWord),
        tTimeout := T#2S);
    IF NOT fbReadStatus.bBusy THEN
        IF fbReadStatus.bError THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbReadStatus.nErrorId;
        END_IF
        step := 0;
    END_IF

100:
    fbWriteSetpoint(
        bWrite := TRUE,
        sNetId := GVL_truST_Test.TargetNetId,
        nPort := GVL_truST_Test.TargetPort,
        sVarName := 'global.Setpoint',
        nSrcAddr := ADR(GVL_truST_Test.SetpointWrite),
        nLen := SIZEOF(GVL_truST_Test.SetpointWrite),
        tTimeout := T#2S);
    IF NOT fbWriteSetpoint.bBusy THEN
        IF fbWriteSetpoint.bError THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbWriteSetpoint.nErrorId;
            step := 0;
        ELSE
            step := 110;
        END_IF
    END_IF

110:
    fbReadSetpoint(
        bRead := TRUE,
        sNetId := GVL_truST_Test.TargetNetId,
        nPort := GVL_truST_Test.TargetPort,
        sVarName := 'global.Setpoint',
        nDestAddr := ADR(GVL_truST_Test.SetpointReadback),
        nLen := SIZEOF(GVL_truST_Test.SetpointReadback),
        tTimeout := T#2S);
    IF NOT fbReadSetpoint.bBusy THEN
        IF fbReadSetpoint.bError THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbReadSetpoint.nErrorId;
        END_IF
        step := 0;
    END_IF

200:
    fbSumRead(
        NETID := GVL_truST_Test.TargetNetId,
        PORT := GVL_truST_Test.TargetPort,
        IDXGRP := 16#F080,
        IDXOFFS := 2,
        WRITELEN := SIZEOF(sumReq),
        READLEN := SIZEOF(sumResp),
        SRCADDR := ADR(sumReq),
        DESTADDR := ADR(sumResp),
        WRTRD := TRUE,
        TMOUT := T#2S);
    IF NOT fbSumRead.BUSY THEN
        IF fbSumRead.ERR THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbSumRead.ERRID;
        ELSE
            GVL_truST_Test.SumResult0 := sumResp.Result0;
            GVL_truST_Test.SumResult1 := sumResp.Result1;
            GVL_truST_Test.SumSetpoint := sumResp.Setpoint;
            GVL_truST_Test.SumTankLevel := sumResp.TankLevel;
            GVL_truST_Test.SumReadOk := (sumResp.Result0 = 0) AND (sumResp.Result1 = 0);
        END_IF
        step := 0;
    END_IF

300:
    fbSumReadWrite(
        NETID := GVL_truST_Test.TargetNetId,
        PORT := GVL_truST_Test.TargetPort,
        IDXGRP := 16#F082,
        IDXOFFS := 2,
        WRITELEN := SIZEOF(sumRwReq),
        READLEN := SIZEOF(sumRwResp),
        SRCADDR := ADR(sumRwReq),
        DESTADDR := ADR(sumRwResp),
        WRTRD := TRUE,
        TMOUT := T#2S);
    IF NOT fbSumReadWrite.BUSY THEN
        IF fbSumReadWrite.ERR THEN
            GVL_truST_Test.AnyError := TRUE;
            GVL_truST_Test.LastErrorId := fbSumReadWrite.ERRID;
        ELSE
            GVL_truST_Test.SumRwResult0 := sumRwResp.Result0;
            GVL_truST_Test.SumRwLength0 := sumRwResp.Length0;
            GVL_truST_Test.SumRwResult1 := sumRwResp.Result1;
            GVL_truST_Test.SumRwLength1 := sumRwResp.Length1;
            GVL_truST_Test.SumRwSetpoint := sumRwResp.Setpoint;
            GVL_truST_Test.SumRwTankLevel := sumRwResp.TankLevel;
            GVL_truST_Test.SumReadWriteOk :=
                (sumRwResp.Result0 = 0)
                AND (sumRwResp.Length0 = SIZEOF(GVL_truST_Test.SumRwSetpoint))
                AND (sumRwResp.Result1 = 0)
                AND (sumRwResp.Length1 = SIZEOF(GVL_truST_Test.SumRwTankLevel));
        END_IF
        step := 0;
    END_IF
END_CASE
```

Online, set `GVL_truST_Test.CmdReadAll := TRUE` for a read pass. Set
`GVL_truST_Test.SetpointWrite`, then set `CmdWriteSetpoint := TRUE` for a guarded
write and read-back pass. Set `CmdSumRead := TRUE` for a `SUMUP_READ` pass; it
should set `SumReadOk = TRUE` and copy both `global.Setpoint` and
`global.TankLevel` in one ADS read-write command. Set `CmdSumReadWrite := TRUE`
for a `SUMUP_READWRITE` response-layout proof; it should set
`SumReadWriteOk = TRUE` and report `SumRwLength0 = 4`, `SumRwLength1 = 4`. If
`LastErrorId` becomes `1797` or `1798`, first check that the route is present,
the truST runtime is running, and the command bit was not left permanently true.

Plain ADS is cleartext and route-based. Keep it on a trusted OT segment.
