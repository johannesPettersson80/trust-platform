use super::*;

/// Named mock scenarios required by the onboarding failure-mode suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockAdsOnboardingScenario {
    Healthy,
    WrongIp,
    DirectedIdentifyBlocked,
    LoopbackIdentifyBlocked,
    WrongAmsNetId,
    MissingRoute,
    FirewallBlocked,
    WrongPlcPort,
    SecureRequired,
    EmptySymbols,
    NotificationFailure,
}

/// Deterministic mock implementation for control-plane tests.
#[derive(Debug, Clone)]
pub struct MockAdsOnboardingWire {
    scenario: MockAdsOnboardingScenario,
    target: TargetIdentity,
}

impl MockAdsOnboardingWire {
    pub fn new(scenario: MockAdsOnboardingScenario) -> Self {
        Self {
            scenario,
            target: TargetIdentity {
                name: Some("CX-1234".to_string()),
                ip: "192.168.10.5".to_string(),
                ams_net_id: "5.23.91.12.1.1".to_string(),
                ams_port: 851,
                tc_version: Some("3.1.4024".to_string()),
            },
        }
    }

    fn fail(
        kind: OnboardingWireErrorKind,
        detail: impl Into<String>,
    ) -> Result<(), OnboardingWireError> {
        Err(OnboardingWireError::new(kind, detail))
    }

    fn sample_symbol() -> SymbolDescriptor {
        SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)
        .with_flag(SymbolFlag::Write)
    }
}

impl Default for MockAdsOnboardingWire {
    fn default() -> Self {
        Self::new(MockAdsOnboardingScenario::Healthy)
    }
}

impl AdsOnboardingWire for MockAdsOnboardingWire {
    fn udp_identify(&mut self, target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
        if matches!(
            self.scenario,
            MockAdsOnboardingScenario::WrongIp | MockAdsOnboardingScenario::DirectedIdentifyBlocked
        ) || (self.scenario == MockAdsOnboardingScenario::LoopbackIdentifyBlocked
            && matches!(target_ip, "127.0.0.1" | "localhost"))
        {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                "target did not answer UDP identify",
            ));
        }
        Ok(self.target.clone())
    }

    fn udp_identify_all(
        &mut self,
        target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::DirectedIdentifyBlocked {
            return Ok(vec![self.target.clone()]);
        }
        self.udp_identify(target_ip).map(|target| vec![target])
    }

    fn tcp_probe_48898(&mut self, _target_ip: &str) -> Result<(), OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::FirewallBlocked {
            return Self::fail(
                OnboardingWireErrorKind::Tcp48898Blocked,
                "TCP 48898 did not accept a connection",
            );
        }
        Ok(())
    }

    fn check_route(
        &mut self,
        _target: &TargetIdentity,
        _local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::MissingRoute {
            return Self::fail(
                OnboardingWireErrorKind::RouteMissing,
                "target rejected route-back identity",
            );
        }
        Ok(())
    }

    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::WrongAmsNetId
            || target.ams_net_id != self.target.ams_net_id
        {
            return Self::fail(
                OnboardingWireErrorKind::WrongAmsNetId,
                "target AMS Net ID did not match",
            );
        }
        Ok(())
    }

    fn read_state(&mut self, _target: &TargetIdentity) -> Result<String, OnboardingWireError> {
        match self.scenario {
            MockAdsOnboardingScenario::WrongPlcPort => Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "PLC runtime port did not respond",
            )),
            MockAdsOnboardingScenario::SecureRequired => Err(OnboardingWireError::new(
                OnboardingWireErrorKind::SecureRequired,
                "target requires Secure ADS",
            )),
            _ => Ok("run".to_string()),
        }
    }

    fn upload_symbols(
        &mut self,
        _target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::EmptySymbols {
            return Ok(Vec::new());
        }
        Ok(vec![Self::sample_symbol()])
    }

    fn resolve_handle(
        &mut self,
        _target: &TargetIdentity,
        symbol: &str,
    ) -> Result<u32, OnboardingWireError> {
        if symbol.is_empty() {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::NoSymbols,
                "symbol name was empty",
            ));
        }
        Ok(42)
    }

    fn sumup_read(
        &mut self,
        _target: &TargetIdentity,
        handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError> {
        Ok(handles.iter().map(|_| vec![0, 0, 0, 0]).collect())
    }

    fn guarded_write_probe(
        &mut self,
        _target: &TargetIdentity,
        _probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError> {
        Ok(())
    }

    fn subscribe_notification(
        &mut self,
        _target: &TargetIdentity,
        _symbol: &str,
    ) -> Result<(), OnboardingWireError> {
        if self.scenario == MockAdsOnboardingScenario::NotificationFailure {
            return Self::fail(
                OnboardingWireErrorKind::NotificationFailure,
                "notification sample was not delivered",
            );
        }
        Ok(())
    }

    fn symbol_version(&mut self, _target: &TargetIdentity) -> Result<u32, OnboardingWireError> {
        Ok(1)
    }

    fn add_route(&mut self, _request: &RouteAddRequest) -> Result<(), OnboardingWireError> {
        Ok(())
    }

    fn remove_route(&mut self, _request: &RouteRemoveRequest) -> Result<(), OnboardingWireError> {
        Ok(())
    }
}
