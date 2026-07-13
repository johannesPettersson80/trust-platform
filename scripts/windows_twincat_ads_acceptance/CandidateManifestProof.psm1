Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1')

function Read-WindowsAdsCandidateManifest {
    param([Parameter(Mandatory = $true)][string]$Path)
    $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    $file = Get-FileEvidence -Path $resolved
    try {
        $manifest = Get-Content -LiteralPath $resolved -Raw -ErrorAction Stop |
            ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "Candidate manifest is not valid JSON: $($_.Exception.Message)"
    }
    $candidate = [string](Get-ObjectProperty $manifest 'candidate_commit_sha')
    $version = [string](Get-ObjectProperty $manifest 'version')
    $vsixHash = [string](Get-ObjectProperty $manifest 'vsix_sha256')
    $vsixName = [string](Get-ObjectProperty $manifest 'vsix_filename')
    $artifactName = [string](Get-ObjectProperty $manifest 'artifact_name')
    $size = [Int64](Get-ObjectProperty $manifest 'vsix_size_bytes')
    $workflow = Get-ObjectProperty $manifest 'workflow_provenance'
    $repository = [string](Get-ObjectProperty $workflow 'repository')
    $workflowPath = [string](Get-ObjectProperty $workflow 'workflow_path')
    $workflowRunId = [Int64](Get-ObjectProperty $workflow 'workflow_run_id')
    $workflowRunAttempt = [Int64](Get-ObjectProperty $workflow 'workflow_run_attempt')
    $workflowRunHeadSha = [string](Get-ObjectProperty $workflow 'workflow_run_head_sha')
    $workflowEvent = [string](Get-ObjectProperty $workflow 'workflow_event')
    $candidateSource = [string](Get-ObjectProperty $workflow 'candidate_source')
    $jobName = [string](Get-ObjectProperty $workflow 'job_name')
    if ((Get-ObjectProperty $manifest 'schema_version') -ne 2 -or
        (Get-ObjectProperty $manifest 'artifact_kind') -ne 'windows_ads_msvc_candidate' -or
        (Get-ObjectProperty $manifest 'target_platform') -ne 'win32-x64' -or
        $candidate -cnotmatch '^[0-9a-f]{40}$' -or
        $artifactName -cne "windows-ads-msvc-candidate-$candidate" -or
        $version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$' -or
        $vsixHash -cnotmatch '^[0-9a-f]{64}$' -or
        [string]::IsNullOrWhiteSpace($vsixName) -or
        $size -le 0 -or
        $repository -cnotmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$' -or
        $workflowPath -cne '.github/workflows/ci.yml' -or
        $workflowRunId -le 0 -or $workflowRunAttempt -le 0 -or
        $workflowRunHeadSha -cnotmatch '^[0-9a-f]{40}$' -or
        $workflowEvent -cnotmatch '^(push|pull_request)$' -or
        $candidateSource -cnotmatch '^(workflow_head|pull_request_head)$' -or
        $jobName -cne 'Windows Packaged Simulator + Native ADS/TcAdsDll Contract' -or
        $candidate -cne $workflowRunHeadSha -or
        ($candidateSource -ceq 'workflow_head' -and $workflowEvent -cne 'push') -or
        ($candidateSource -ceq 'pull_request_head' -and $workflowEvent -cne 'pull_request')) {
        throw 'Candidate manifest identity is incomplete or invalid.'
    }
    return [pscustomobject][ordered]@{
        path = $resolved
        manifest_sha256 = $file.sha256
        candidate_commit_sha = $candidate
        artifact_name = $artifactName
        version = $version
        target_platform = 'win32-x64'
        vsix_filename = $vsixName
        vsix_sha256 = $vsixHash
        vsix_size_bytes = $size
        workflow_provenance = [pscustomobject][ordered]@{
            repository = $repository
            workflow_path = $workflowPath
            workflow_run_id = $workflowRunId
            workflow_run_attempt = $workflowRunAttempt
            workflow_run_head_sha = $workflowRunHeadSha
            workflow_event = $workflowEvent
            candidate_source = $candidateSource
            job_name = $jobName
        }
    }
}

function Assert-WindowsAdsCandidateVsix {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$SourcePath,
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)]$Provenance,
        [Parameter(Mandatory = $true)][string]$ActualVersion,
        [Parameter(Mandatory = $true)][string]$ActualTargetPlatform
    )
    if ((Split-Path -Leaf $SourcePath) -cne $Manifest.vsix_filename -or
        $Snapshot.sha256 -cne $Manifest.vsix_sha256 -or
        $Snapshot.size_bytes -ne $Manifest.vsix_size_bytes -or
        $ActualVersion -cne $Manifest.version -or
        $ActualTargetPlatform -cne $Manifest.target_platform -or
        $Provenance.candidate_manifest_sha256 -cne $Manifest.manifest_sha256 -or
        $Provenance.vsix_sha256 -cne $Manifest.vsix_sha256 -or
        $Provenance.workflow_run_id -ne $Manifest.workflow_provenance.workflow_run_id -or
        $Provenance.artifact_name -cne $Manifest.artifact_name) {
        throw 'Selected VSIX filename, bytes, version, or platform differs from the CI candidate manifest.'
    }
    return [pscustomobject][ordered]@{
        sha256 = $Manifest.manifest_sha256
        candidate_commit_sha = $Manifest.candidate_commit_sha
        artifact_name = $Manifest.artifact_name
        version = $Manifest.version
        target_platform = $Manifest.target_platform
        vsix_filename = $Manifest.vsix_filename
        vsix_sha256 = $Manifest.vsix_sha256
        vsix_size_bytes = $Manifest.vsix_size_bytes
        workflow_provenance = $Manifest.workflow_provenance
    }
}

Export-ModuleMember -Function @(
    'Read-WindowsAdsCandidateManifest',
    'Assert-WindowsAdsCandidateVsix'
)
