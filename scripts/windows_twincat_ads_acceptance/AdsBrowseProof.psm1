Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function Test-AdsSymbolTreeNode {
    param([Parameter(Mandatory = $true)]$Node)
    if ($null -eq $Node) { return $false }
    foreach ($name in @('id', 'name', 'path', 'type')) {
        $value = Get-ObjectProperty -Object $Node -Name $name
        if (-not ($value -is [string]) -or [string]::IsNullOrWhiteSpace($value)) {
            return $false
        }
    }
    $properties = @($Node.PSObject.Properties | ForEach-Object { $_.Name })
    if ($properties -contains 'children') {
        $childrenValue = Get-ObjectProperty -Object $Node -Name 'children'
        if ($null -ne $childrenValue -and -not ($childrenValue -is [Array])) {
            return $false
        }
        foreach ($child in @($childrenValue)) {
            if ($null -ne $child -and -not (Test-AdsSymbolTreeNode -Node $child)) {
                return $false
            }
        }
    }
    return $true
}

function Get-AdsBrowseResponseContractError {
    param([Parameter(Mandatory = $true)]$Response)
    $properties = @($Response.PSObject.Properties | ForEach-Object { $_.Name })
    if ($properties -notcontains 'schema_version' -or
        [int](Get-ObjectProperty $Response 'schema_version') -ne 1) {
        return 'ADS browse response has an unsupported or missing schema version.'
    }
    if ((Get-ObjectProperty $Response 'protocol') -cne 'ads' -or
        (Get-ObjectProperty $Response 'kind') -cne 'symbols') {
        return 'ADS browse response has the wrong protocol or result kind.'
    }
    if ($properties -notcontains 'tree') {
        return 'ADS browse response has no symbol tree.'
    }
    $treeValue = Get-ObjectProperty $Response 'tree'
    if ($null -ne $treeValue -and -not ($treeValue -is [Array])) {
        return 'ADS browse response symbol tree is not an array.'
    }
    foreach ($node in @($treeValue)) {
        if ($null -ne $node -and -not (Test-AdsSymbolTreeNode -Node $node)) {
            return 'ADS browse response contains an invalid symbol tree node.'
        }
    }
    if ($properties -contains 'error') {
        $errorObject = Get-ObjectProperty $Response 'error'
        if ($null -eq $errorObject -or
            [string]::IsNullOrWhiteSpace([string](Get-ObjectProperty $errorObject 'code')) -or
            [string]::IsNullOrWhiteSpace([string](Get-ObjectProperty $errorObject 'message'))) {
            return 'ADS browse response has invalid structured error evidence.'
        }
    }
    if ($properties -contains 'route') {
        $route = Get-ObjectProperty $Response 'route'
        if ($null -eq $route -or
            @($route.PSObject.Properties | ForEach-Object { $_.Name }) -notcontains 'status' -or
            [string]::IsNullOrWhiteSpace([string](Get-ObjectProperty $route 'status'))) {
            return 'ADS browse response has invalid route evidence.'
        }
    }
    return $null
}

function New-AdsProbeContractFailure {
    param(
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Message,
        [Parameter(Mandatory = $true)]$CommandEvidence
    )
    return [pscustomobject][ordered]@{
        port = $Port
        status = 'check_failed'
        responded = $false
        browsable = $false
        symbol_count = 0
        error = [pscustomobject][ordered]@{
            code = 'invalid_browse_response'
            message = $Message
        }
        command = $CommandEvidence
    }
}

function Get-AdsBrowseProbeClassification {
    param(
        [Parameter()][AllowNull()]$ErrorObject,
        [Parameter(Mandatory = $true)][int]$SymbolCount
    )
    if ($null -eq $ErrorObject -and $SymbolCount -gt 0) {
        return [pscustomobject]@{ status = 'available'; responded = $true; browsable = $true; error = $null }
    }
    if ($null -eq $ErrorObject) {
        $ErrorObject = [pscustomobject][ordered]@{
            code = 'unexplained_empty_browse_response'
            message = 'ADS browse returned no variables and no explicit empty-service result.'
        }
    }
    $code = [string](Get-ObjectProperty $ErrorObject 'code')
    $status = if ($code -eq 'symbol_upload_unsupported') { 'unsupported' } `
        elseif ($code -eq 'empty_symbol_table') { 'empty' } `
        elseif ($code -eq 'ads_port_unavailable') { 'unavailable' } `
        else { 'check_failed' }
    return [pscustomobject]@{
        status = $status
        responded = $status -in @('unsupported', 'empty')
        browsable = $false
        error = $ErrorObject
    }
}

function Assert-AmsNetId {
    param([Parameter(Mandatory = $true)][string]$Value)
    $parts = @($Value.Split('.'))
    if ($parts.Count -ne 6) { throw "Native discovery returned invalid AMS Net ID '$Value'" }
    $octets = New-Object 'System.Collections.Generic.List[int]'
    foreach ($part in $parts) {
        $parsed = 0
        if (-not [int]::TryParse($part, [ref]$parsed) -or $parsed -lt 0 -or $parsed -gt 255) {
            throw "Native discovery returned invalid AMS Net ID '$Value'"
        }
        [void]$octets.Add($parsed)
    }
    if (@($octets | Where-Object { $_ -ne 0 }).Count -eq 0) {
        throw 'Native discovery returned the empty AMS Net ID 0.0.0.0.0.0'
    }
}

function Get-DoctorStep {
    param(
        [Parameter(Mandatory = $true)]$Report,
        [Parameter(Mandatory = $true)][string]$Id
    )
    $matches = @(@(Get-ObjectProperty $Report 'steps') | Where-Object { (Get-ObjectProperty $_ 'id') -eq $Id })
    if ($matches.Count -ne 1) { throw "ADS doctor report contained $($matches.Count) '$Id' steps" }
    return $matches[0]
}

function Assert-NativeDoctorProof {
    param(
        [Parameter(Mandatory = $true)]$Report,
        [Parameter(Mandatory = $true)]$Candidate,
        [Parameter(Mandatory = $true)][string]$ExpectedTargetNetId,
        [Parameter()][string]$ExpectedRouterSourceNetId,
        [Parameter(Mandatory = $true)]$CommandEvidence,
        [Parameter(Mandatory = $true)][string]$ReportHash
    )
    if ((Get-ObjectProperty $Report 'writes_enabled') -ne $false) {
        throw 'ADS doctor unexpectedly enabled writes'
    }
    $target = Get-ObjectProperty $Report 'target'
    if ($Candidate.ams_net_id -cne $ExpectedTargetNetId -or
        (Get-ObjectProperty $target 'ams_net_id') -cne $ExpectedTargetNetId -or
        (Get-ObjectProperty $target 'ip') -ne $Candidate.host -or
        [int](Get-ObjectProperty $target 'ams_port') -ne 851) {
        throw 'ADS doctor did not preserve the proven native candidate and port 851'
    }
    foreach ($id in @('udp_identify', 'local_identity', 'tcp_48898', 'route_present', 'ams_target', 'read_state', 'symbol_upload', 'handle_resolve', 'sumup_read', 'notification', 'symbol_version')) {
        $step = Get-DoctorStep -Report $Report -Id $id
        if ((Get-ObjectProperty $step 'status') -ne 'pass') {
            throw "ADS doctor step '$id' did not pass: $([string](Get-ObjectProperty $step 'detail'))"
        }
    }
    $transport = Get-DoctorStep -Report $Report -Id 'tcp_48898'
    $transportEvidence = Get-ObjectProperty $transport 'evidence'
    if ((Get-ObjectProperty $transportEvidence 'probe_transport') -ne 'native_windows_router' -or
        (Get-ObjectProperty $transportEvidence 'probe_operation') -ne 'read_state') {
        throw 'ADS doctor did not prove a native Windows router read-state round trip'
    }
    if ((Get-ObjectProperty $transportEvidence 'source_ams_address_available') -ne $true -or
        (Get-ObjectProperty $transportEvidence 'source_target_comparison') -ne 'full_ams_address' -or
        (Get-ObjectProperty $transportEvidence 'source_target_addresses_distinct') -ne $true) {
        throw 'ADS doctor did not prove distinct full native source and target AMS addresses'
    }
    $sourceNetId = [string](Get-ObjectProperty $transportEvidence 'source_ams_net_id')
    $targetNetId = [string](Get-ObjectProperty $transportEvidence 'target_ams_net_id')
    $sourcePort = [int](Get-ObjectProperty $transportEvidence 'source_ams_port')
    $targetPort = [int](Get-ObjectProperty $transportEvidence 'target_ams_port')
    Assert-AmsNetId -Value $sourceNetId
    Assert-AmsNetId -Value $targetNetId
    if ($sourcePort -lt 1 -or $sourcePort -gt 65535 -or
        $targetPort -lt 1 -or $targetPort -gt 65535) {
        throw 'ADS doctor reported an invalid source or target AMS port'
    }
    if ($targetNetId -cne $ExpectedTargetNetId -or $targetPort -ne 851) {
        throw 'ADS doctor native transport evidence did not preserve the expected target AMS address'
    }
    if ($sourceNetId -ceq $targetNetId -and $sourcePort -eq $targetPort) {
        throw 'ADS doctor conflated the native router source and target AMS addresses'
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedRouterSourceNetId) -and
        $sourceNetId -cne $ExpectedRouterSourceNetId) {
        throw "ADS doctor native router source AMS identity '$sourceNetId' differs from expected '$ExpectedRouterSourceNetId'"
    }
    $route = Get-DoctorStep -Report $Report -Id 'route_present'
    $routeEvidence = Get-ObjectProperty $route 'evidence'
    if ((Get-ObjectProperty $routeEvidence 'route_mode') -ne 'native_local_no_self_route') {
        throw 'ADS doctor did not prove native_local_no_self_route'
    }
    if ((Get-ObjectProperty $routeEvidence 'source_ams_net_id') -cne $sourceNetId -or
        [int](Get-ObjectProperty $routeEvidence 'source_ams_port') -ne $sourcePort -or
        (Get-ObjectProperty $routeEvidence 'target_ams_net_id') -cne $targetNetId -or
        [int](Get-ObjectProperty $routeEvidence 'target_ams_port') -ne $targetPort -or
        (Get-ObjectProperty $routeEvidence 'source_target_addresses_distinct') -ne $true -or
        (Get-ObjectProperty $routeEvidence 'source_target_comparison') -ne 'full_ams_address') {
        throw 'ADS doctor route evidence did not preserve the proven full source and target AMS addresses'
    }
    $notification = Get-DoctorStep -Report $Report -Id 'notification'
    $notificationEvidence = Get-ObjectProperty $notification 'evidence'
    $notificationSubscriptionId = [Int64](Get-ObjectProperty $notificationEvidence 'subscription_id')
    if ((Get-ObjectProperty $notificationEvidence 'read_proven') -ne $true -or
        (Get-ObjectProperty $notificationEvidence 'sample_method') -ne 'subscribed_read_update' -or
        $notificationSubscriptionId -le 0) {
        throw 'ADS doctor notification step did not prove a drained subscribed read update'
    }
    foreach ($step in @(Get-ObjectProperty $Report 'steps')) {
        $nextAction = Get-ObjectProperty $step 'next_action'
        if ($null -ne $nextAction -and (Get-ObjectProperty $nextAction 'kind') -eq 'add_route') {
            throw "ADS doctor step '$([string](Get-ObjectProperty $step 'id'))' proposed a forbidden self-route"
        }
    }
    $write = Get-DoctorStep -Report $Report -Id 'write_guarded'
    if ((Get-ObjectProperty $write 'status') -ne 'skip' -or
        (Get-ObjectProperty $write 'skip_reason') -ne 'writes_disabled') {
        throw 'ADS doctor did not prove that the guarded write step stayed disabled'
    }
    return [pscustomobject][ordered]@{
        native_reply_proven = $true
        target_ams_net_id = $Candidate.ams_net_id
        target_ams_port = $targetPort
        target_ads_port = $targetPort
        source_ams_net_id = $sourceNetId
        source_ams_port = $sourcePort
        source_target_addresses_distinct = $true
        probe_transport = 'native_windows_router'
        probe_operation = 'read_state'
        route_mode = 'native_local_no_self_route'
        notification_read_proven = $true
        notification_sample_method = 'subscribed_read_update'
        notification_subscription_id = $notificationSubscriptionId
        writes_enabled = $false
        write_step = 'writes_disabled'
        overall = Get-ObjectProperty $Report 'overall'
        report_sha256 = $ReportHash
        command = $CommandEvidence
    }
}

function Invoke-Doctor851 {
    param(
        [Parameter(Mandatory = $true)][string]$Runtime,
        [Parameter(Mandatory = $true)]$Candidate,
        [Parameter(Mandatory = $true)][string]$ExpectedTargetNetId,
        [Parameter()][string]$ExpectedRouterSourceNetId,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    $arguments = @('ads', 'doctor', '--target', $Candidate.host, '--target-net-id', $Candidate.ams_net_id, '--ams-port', '851', '--json')
    $result = Invoke-CapturedProcess -FilePath $Runtime -Arguments $arguments -TimeoutSeconds $TimeoutSeconds
    $report = Convert-CommandJson -Result $result -Context "ADS doctor for $($Candidate.ams_net_id) port 851"
    $commandEvidence = New-CommandEvidence $result
    $reportHash = Get-StringSha256 $result.stdout
    return Assert-NativeDoctorProof -Report $report -Candidate $Candidate `
        -ExpectedTargetNetId $ExpectedTargetNetId `
        -ExpectedRouterSourceNetId $ExpectedRouterSourceNetId `
        -CommandEvidence $commandEvidence `
        -ReportHash $reportHash
}

Export-ModuleMember -Function @(
    'Assert-AmsNetId',
    'Assert-NativeDoctorProof',
    'Invoke-Doctor851',
    'Get-AdsBrowseResponseContractError',
    'New-AdsProbeContractFailure',
    'Get-AdsBrowseProbeClassification'
)
