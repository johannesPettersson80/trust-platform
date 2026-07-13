Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'AcceptanceIo.psm1') -Force

function Read-WindowsAdsCandidateProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$CandidateManifest
    )
    $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    $file = Get-FileEvidence -Path $resolved
    try {
        $proof = Get-Content -LiteralPath $resolved -Raw -ErrorAction Stop |
            ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "Candidate provenance is not valid JSON: $($_.Exception.Message)"
    }
    $archiveName = [string](Get-ObjectProperty $proof 'artifact_archive_filename')
    if ([IO.Path]::GetFileName($archiveName) -cne $archiveName -or
        $archiveName -cnotmatch '^[A-Za-z0-9_.-]+[.]zip$') {
        throw 'Candidate provenance archive filename is unsafe.'
    }
    $archivePath = Join-Path (Split-Path -Parent $resolved) $archiveName
    $archive = Get-FileEvidence -Path $archivePath
    $verification = Get-ObjectProperty $proof 'verification'
    $candidateManifestHash = [string](Get-ObjectProperty $proof 'candidate_manifest_sha256')
    $vsixHash = [string](Get-ObjectProperty $proof 'vsix_sha256')
    $archiveHash = [string](Get-ObjectProperty $proof 'artifact_archive_sha256')
    $artifactName = [string](Get-ObjectProperty $proof 'artifact_name')
    $candidate = [string](Get-ObjectProperty $proof 'candidate_commit_sha')
    $runId = [Int64](Get-ObjectProperty $proof 'workflow_run_id')
    $artifactId = [Int64](Get-ObjectProperty $proof 'artifact_id')
    if ((Get-ObjectProperty $proof 'schema_version') -ne 1 -or
        (Get-ObjectProperty $proof 'provenance_kind') -ne 'github_actions_artifact_api_v1' -or
        (Get-ObjectProperty $proof 'repository') -cne $CandidateManifest.workflow_provenance.repository -or
        (Get-ObjectProperty $proof 'workflow_path') -cne $CandidateManifest.workflow_provenance.workflow_path -or
        $runId -ne $CandidateManifest.workflow_provenance.workflow_run_id -or
        (Get-ObjectProperty $proof 'workflow_run_attempt') -ne $CandidateManifest.workflow_provenance.workflow_run_attempt -or
        (Get-ObjectProperty $proof 'workflow_run_head_sha') -cne $CandidateManifest.workflow_provenance.workflow_run_head_sha -or
        (Get-ObjectProperty $proof 'workflow_event') -cne $CandidateManifest.workflow_provenance.workflow_event -or
        (Get-ObjectProperty $proof 'candidate_source') -cne $CandidateManifest.workflow_provenance.candidate_source -or
        (Get-ObjectProperty $proof 'job_name') -cne $CandidateManifest.workflow_provenance.job_name -or
        $candidate -cne $CandidateManifest.candidate_commit_sha -or
        $artifactName -cne $CandidateManifest.artifact_name -or
        $archiveName -cne "$artifactName.zip" -or
        $artifactId -le 0 -or
        (Get-ObjectProperty $proof 'candidate_manifest_filename') -cne (Split-Path -Leaf $CandidateManifest.path) -or
        $candidateManifestHash -cne $CandidateManifest.manifest_sha256 -or
        $vsixHash -cne $CandidateManifest.vsix_sha256 -or
        (Get-ObjectProperty $proof 'vsix_filename') -cne $CandidateManifest.vsix_filename -or
        (Get-ObjectProperty $proof 'vsix_size_bytes') -ne $CandidateManifest.vsix_size_bytes -or
        $archiveHash -cnotmatch '^[0-9a-f]{64}$' -or
        $archive.sha256 -cne $archiveHash -or
        $archive.size_bytes -ne (Get-ObjectProperty $proof 'artifact_archive_size_bytes') -or
        (Get-ObjectProperty $verification 'github_api_run_exact') -ne $true -or
        (Get-ObjectProperty $verification 'github_api_job_success') -ne $true -or
        (Get-ObjectProperty $verification 'github_api_artifact_exact') -ne $true -or
        (Get-ObjectProperty $verification 'artifact_archive_digest_verified') -ne $true -or
        (Get-ObjectProperty $verification 'offline_bundle_integrity_ready') -ne $true) {
        throw 'Candidate provenance does not bind the manifest, VSIX, successful job, API artifact, and archive digest.'
    }
    return [pscustomobject][ordered]@{
        sha256 = $file.sha256
        repository = $CandidateManifest.workflow_provenance.repository
        workflow_path = $CandidateManifest.workflow_provenance.workflow_path
        workflow_run_id = $runId
        workflow_run_attempt = $CandidateManifest.workflow_provenance.workflow_run_attempt
        workflow_run_head_sha = $CandidateManifest.workflow_provenance.workflow_run_head_sha
        workflow_event = $CandidateManifest.workflow_provenance.workflow_event
        candidate_source = $CandidateManifest.workflow_provenance.candidate_source
        candidate_commit_sha = $candidate
        job_name = $CandidateManifest.workflow_provenance.job_name
        artifact_id = $artifactId
        artifact_name = $artifactName
        artifact_archive_sha256 = $archiveHash
        candidate_manifest_sha256 = $candidateManifestHash
        vsix_sha256 = $vsixHash
    }
}

Export-ModuleMember -Function 'Read-WindowsAdsCandidateProvenance'
