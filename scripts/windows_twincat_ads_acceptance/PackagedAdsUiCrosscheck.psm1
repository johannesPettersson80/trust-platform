Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function Test-PackagedAdsUiAgainstCliServices {
    param(
        [Parameter(Mandatory = $true)]$SimulatorEvidence,
        [Parameter(Mandatory = $true)][object[]]$CliTargets,
        [Parameter(Mandatory = $true)][string]$ExpectedTargetNetId,
        [Parameter(Mandatory = $true)][int[]]$ExpectedAdsPorts
    )
    $extensionHost = Get-ObjectProperty -Object $SimulatorEvidence -Name 'extension_host'
    $journey = if ($null -eq $extensionHost) { $null } else {
        Get-ObjectProperty -Object $extensionHost -Name 'journey'
    }
    $adsUi = if ($null -eq $journey) { $null } else {
        Get-ObjectProperty -Object $journey -Name 'ads_ui'
    }
    $uiTarget = if ($null -eq $adsUi) { $null } else {
        Get-ObjectProperty -Object $adsUi -Name 'discovered_target'
    }
    $imported = if ($null -eq $adsUi) { $null } else {
        Get-ObjectProperty -Object $adsUi -Name 'imported_variable'
    }
    $artifacts = if ($null -eq $imported) { $null } else {
        Get-ObjectProperty -Object $imported -Name 'artifacts'
    }
    $liveValues = if ($null -eq $adsUi) { $null } else {
        Get-ObjectProperty -Object $adsUi -Name 'live_values'
    }
    $customRecovery = if ($null -eq $adsUi) { $null } else {
        Get-ObjectProperty -Object $adsUi -Name 'custom_port_recovery'
    }
    $uiStatus = if ($null -eq $adsUi) { $null } else {
        [string](Get-ObjectProperty -Object $adsUi -Name 'status')
    }
    $uiNetId = if ($null -eq $uiTarget) { $null } else {
        [string](Get-ObjectProperty -Object $uiTarget -Name 'ams_net_id')
    }
    $uiHost = if ($null -eq $uiTarget) { $null } else {
        [string](Get-ObjectProperty -Object $uiTarget -Name 'host')
    }
    $matchingTargets = @($CliTargets | Where-Object {
        $candidate = Get-ObjectProperty -Object $_ -Name 'candidate'
        $null -ne $candidate -and
            [string](Get-ObjectProperty -Object $candidate -Name 'ams_net_id') -ceq $ExpectedTargetNetId -and
            $uiNetId -ceq $ExpectedTargetNetId
    })
    $cliTarget = if ($matchingTargets.Count -eq 1) { $matchingTargets[0] } else { $null }
    $uiServices = if ($null -eq $uiTarget) { @() } else {
        @(Get-ObjectProperty -Object $uiTarget -Name 'services')
    }
    $customUiServices = if ($null -eq $customRecovery) { @() } else {
        @(Get-ObjectProperty -Object $customRecovery -Name 'result_rows')
    }
    $cliServices = if ($null -eq $cliTarget) { @() } else {
        @(Get-ObjectProperty -Object $cliTarget -Name 'services')
    }
    $cliCandidate = if ($null -eq $cliTarget) { $null } else {
        Get-ObjectProperty -Object $cliTarget -Name 'candidate'
    }
    $cliHost = if ($null -eq $cliCandidate) { $null } else {
        [string](Get-ObjectProperty -Object $cliCandidate -Name 'host')
    }
    $hostMatches = -not [string]::IsNullOrWhiteSpace($uiHost) -and $uiHost -ceq $cliHost
    $builtInPorts = @(851, 852, 853, 854, 301, 501)
    $expectedPorts = @($ExpectedAdsPorts)
    $expectedCustomPorts = @($expectedPorts | Where-Object { $builtInPorts -notcontains $_ })
    $requestedCustomPorts = if ($null -eq $customRecovery) { @() } else {
        @(Get-ObjectProperty -Object $customRecovery -Name 'requested_custom_ports')
    }
    $customPortsMatch = $expectedCustomPorts.Count -eq $requestedCustomPorts.Count -and
        @($expectedCustomPorts | Where-Object { $requestedCustomPorts -notcontains $_ }).Count -eq 0
    $mismatches = New-Object 'System.Collections.Generic.List[object]'
    foreach ($port in $expectedPorts) {
        $uiSource = if ($builtInPorts -contains $port) { $uiServices } else { $customUiServices }
        $uiRows = @($uiSource | Where-Object { [int](Get-ObjectProperty -Object $_ -Name 'port') -eq $port })
        $cliRows = @($cliServices | Where-Object { [int](Get-ObjectProperty -Object $_ -Name 'port') -eq $port })
        $matches = $uiRows.Count -eq 1 -and $cliRows.Count -eq 1 -and
            [string](Get-ObjectProperty -Object $uiRows[0] -Name 'status') -eq
                [string](Get-ObjectProperty -Object $cliRows[0] -Name 'status')
        if ($matches -and $port -eq 851) {
            $matches = [int](Get-ObjectProperty -Object $uiRows[0] -Name 'symbol_count') -eq
                [int](Get-ObjectProperty -Object $cliRows[0] -Name 'symbol_count') -and
                [int](Get-ObjectProperty -Object $cliRows[0] -Name 'symbol_count') -gt 0
        }
        if (-not $matches) {
            [void]$mismatches.Add([pscustomobject][ordered]@{
                port = $port
                ui_status = if ($uiRows.Count -eq 1) { Get-ObjectProperty -Object $uiRows[0] -Name 'status' } else { $null }
                cli_status = if ($cliRows.Count -eq 1) { Get-ObjectProperty -Object $cliRows[0] -Name 'status' } else { $null }
            })
        }
    }
    $matchesNativeCli = $uiNetId -ceq $ExpectedTargetNetId -and $hostMatches -and
        $matchingTargets.Count -eq 1 -and $customPortsMatch -and $mismatches.Count -eq 0
    $importedReadOnly = $null -ne $imported -and
        [string](Get-ObjectProperty -Object $imported -Name 'configured_access') -ceq 'read' -and
        (Get-ObjectProperty -Object $imported -Name 'allow_writes') -eq $false -and
        $null -ne $artifacts -and
        (Get-ObjectProperty -Object $artifacts -Name 'ads_toml_present') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'selected_remote_symbol_present') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'selected_point_mapping_exact') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'selected_connection_route_exact') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'generated_st_present') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'generated_typed_local_declaration') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'generated_quality_mapping') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'selected_symbol_snapshot_present') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'selected_snapshot_structural') -eq $true -and
        (Get-ObjectProperty -Object $artifacts -Name 'runtime_ads_enabled') -eq $true
    $liveValuesProven = $null -ne $liveValues -and
        [int](Get-ObjectProperty -Object $liveValues -Name 'schema_version') -eq 1 -and
        (Get-ObjectProperty -Object $liveValues -Name 'response_event_converged') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'response_imported_entry_found') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'event_imported_entry_found') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'accepted_scan_positive') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'accepted_quality_recent_after_restart') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'later_response_event_converged') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'later_scan_strictly_higher') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'later_same_imported_entry') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'later_imported_entry_still_good') -eq $true -and
        [string](Get-ObjectProperty -Object $liveValues -Name 'access') -ceq 'read' -and
        [string](Get-ObjectProperty -Object $liveValues -Name 'quality') -ceq 'good' -and
        (Get-ObjectProperty -Object $liveValues -Name 'rendered') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'rendered_response_event_converged') -eq $true -and
        [int64](Get-ObjectProperty -Object $liveValues -Name 'rendered_dap_scan') -gt 0 -and
        (Get-ObjectProperty -Object $liveValues -Name 'rendered_read_only') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'rendered_without_actions') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'value_matches_dap') -eq $true -and
        (Get-ObjectProperty -Object $liveValues -Name 'type_matches_dap') -eq $true
    return [pscustomobject][ordered]@{
        passed = $uiStatus -eq 'pass' -and $matchesNativeCli -and
            $importedReadOnly -and $liveValuesProven
        evidence = [ordered]@{
            status = $uiStatus
            expected_target_ams_net_id = $ExpectedTargetNetId
            ams_net_id = $uiNetId
            compared_ads_ports = @($expectedPorts)
            advanced_custom_ports_match = $customPortsMatch
            host_matches = $hostMatches
            matches_proven_native_cli_candidate = $matchesNativeCli
            service_statuses_match = $mismatches.Count -eq 0
            imported_read_only_variable = $importedReadOnly
            imported_variable_rendered_in_live_values = $liveValuesProven
            mismatches = @($mismatches.ToArray())
        }
    }
}

Export-ModuleMember -Function 'Test-PackagedAdsUiAgainstCliServices'
