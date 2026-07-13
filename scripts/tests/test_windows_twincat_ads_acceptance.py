from __future__ import annotations

import unittest

from .windows_twincat_ads_acceptance_discovery_contracts import (
    AdsDiscoveryContractsMixin,
)
from .windows_twincat_ads_acceptance_journey_contracts import (
    PackagedAdsJourneyContractsMixin,
)
from .windows_twincat_ads_acceptance_powershell_contracts import (
    PowerShellContractsMixin,
)
from .windows_twincat_ads_acceptance_simulator_contracts import (
    SimulatorContractsMixin,
)
from .windows_twincat_ads_acceptance_support import (
    WindowsTwinCatAdsAcceptanceContractSupport,
)


class WindowsTwinCatAdsAcceptanceContractTests(
    SimulatorContractsMixin,
    PackagedAdsJourneyContractsMixin,
    AdsDiscoveryContractsMixin,
    PowerShellContractsMixin,
    WindowsTwinCatAdsAcceptanceContractSupport,
    unittest.TestCase,
):
    pass


if __name__ == "__main__":
    unittest.main()
