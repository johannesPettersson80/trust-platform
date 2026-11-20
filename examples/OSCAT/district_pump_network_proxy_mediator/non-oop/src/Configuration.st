CONFIGURATION DistrictPumpNetworkProxyMediatorClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Network WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Network.LocalDemandRaw AT %IW2 : INT;
    Network.NorthDemandRaw AT %IW4 : INT;
    Network.EastDemandRaw AT %IW6 : INT;
    Network.NorthAgeSeconds AT %IW8 : INT;
    Network.EastAgeSeconds AT %IW10 : INT;
    Network.NorthQualityGood AT %IX0.0 : BOOL;
    Network.EastQualityGood AT %IX0.1 : BOOL;
    Network.SuctionHealthy AT %IX0.2 : BOOL;
    Network.PumpSpeedRaw AT %QW2 : INT;
    Network.StaleAlarmOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
