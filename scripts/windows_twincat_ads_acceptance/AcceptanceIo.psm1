Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-UtcTimestamp {
    return [DateTime]::UtcNow.ToString('o')
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Path)) | Out-Null
    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Get-ObjectProperty {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )
    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Get-StringSha256 {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes($Value)
        return ([BitConverter]::ToString($algorithm.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function Get-FileEvidence {
    param([Parameter(Mandatory = $true)][string]$Path)
    $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    $stream = [IO.File]::Open(
        $resolved,
        [IO.FileMode]::Open,
        [IO.FileAccess]::Read,
        ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
    )
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $length = $stream.Length
        $hash = ([BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
    return [pscustomobject][ordered]@{
        path = $resolved
        size_bytes = [Int64]$length
        sha256 = $hash
    }
}

function New-FileSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    $resolvedSource = (Resolve-Path -LiteralPath $Source -ErrorAction Stop).Path
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Destination)) | Out-Null
    $input = [IO.File]::Open(
        $resolvedSource,
        [IO.FileMode]::Open,
        [IO.FileAccess]::Read,
        [IO.FileShare]::Read
    )
    $output = [IO.File]::Open(
        $Destination,
        [IO.FileMode]::CreateNew,
        [IO.FileAccess]::Write,
        [IO.FileShare]::None
    )
    try { $input.CopyTo($output) }
    finally { $output.Dispose(); $input.Dispose() }
    return Get-FileEvidence -Path $Destination
}

function ConvertTo-NativeArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)
    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') {
        return $Value
    }
    $builder = New-Object Text.StringBuilder
    [void]$builder.Append('"')
    $slashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $slashes += 1
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($slashes * 2) + 1)))
            [void]$builder.Append('"')
            $slashes = 0
            continue
        }
        if ($slashes -gt 0) {
            [void]$builder.Append(('\' * $slashes))
            $slashes = 0
        }
        [void]$builder.Append($character)
    }
    if ($slashes -gt 0) {
        [void]$builder.Append(('\' * ($slashes * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter()][System.Collections.IDictionary]$EnvironmentOverrides
    )
    $startInfo = New-Object Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = (@($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $utf8 = New-Object Text.UTF8Encoding($false, $false)
    $startInfo.StandardOutputEncoding = $utf8
    $startInfo.StandardErrorEncoding = $utf8
    if ($null -ne $EnvironmentOverrides) {
        foreach ($entry in $EnvironmentOverrides.GetEnumerator()) {
            $name = [string]$entry.Key
            if ([string]::IsNullOrWhiteSpace($name)) {
                throw 'A child process environment override had an empty name.'
            }
            if ($null -eq $entry.Value) {
                $startInfo.EnvironmentVariables.Remove($name)
            }
            else {
                $startInfo.EnvironmentVariables[$name] = [string]$entry.Value
            }
        }
    }

    $process = New-Object Diagnostics.Process
    $process.StartInfo = $startInfo
    $timer = [Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "Failed to start $FilePath"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $timedOut = -not $process.WaitForExit($TimeoutSeconds * 1000)
    $terminated = $process.HasExited
    $taskkillAttempted = $false
    if ($timedOut) {
        try { $process.Kill() } catch { }
        try { $terminated = $process.WaitForExit(2000) } catch { $terminated = $false }
        if (-not $terminated) {
            $taskkillAttempted = $true
            $taskkill = if ([string]::IsNullOrWhiteSpace($env:SystemRoot)) {
                'taskkill.exe'
            } else {
                Join-Path $env:SystemRoot 'System32\taskkill.exe'
            }
            try { & $taskkill /PID $process.Id /T /F | Out-Null } catch { }
            try { $terminated = $process.WaitForExit(10000) } catch { $terminated = $false }
        }
    }
    $timer.Stop()
    $stdout = ''
    $stderr = ''
    if ($terminated) {
        try {
            if ($stdoutTask.Wait(5000)) { $stdout = $stdoutTask.GetAwaiter().GetResult() }
        } catch { }
        try {
            if ($stderrTask.Wait(5000)) { $stderr = $stderrTask.GetAwaiter().GetResult() }
        } catch { }
    }
    $exitCode = if ($timedOut) { -1 } else { $process.ExitCode }
    $process.Dispose()

    return [pscustomobject][ordered]@{
        executable = $FilePath
        arguments = @($Arguments)
        exit_code = $exitCode
        timed_out = $timedOut
        termination_completed = $terminated
        taskkill_fallback_attempted = $taskkillAttempted
        duration_ms = [Int64]$timer.ElapsedMilliseconds
        stdout = $stdout
        stderr = $stderr
    }
}

function New-CommandEvidence {
    param([Parameter(Mandatory = $true)]$Result)
    return [pscustomobject][ordered]@{
        executable = $Result.executable
        arguments = @($Result.arguments)
        exit_code = $Result.exit_code
        timed_out = $Result.timed_out
        duration_ms = $Result.duration_ms
        stdout_size_bytes = [Text.Encoding]::UTF8.GetByteCount($Result.stdout)
        stdout_sha256 = Get-StringSha256 $Result.stdout
        stderr = $Result.stderr.Trim()
    }
}

function Convert-CommandJson {
    param(
        [Parameter(Mandatory = $true)]$Result,
        [Parameter(Mandatory = $true)][string]$Context
    )
    if ($Result.timed_out) {
        throw "$Context timed out after $($Result.duration_ms) ms"
    }
    if ($Result.exit_code -ne 0) {
        $detail = if ([string]::IsNullOrWhiteSpace($Result.stderr)) { $Result.stdout } else { $Result.stderr }
        throw "$Context failed with exit $($Result.exit_code): $($detail.Trim())"
    }
    try {
        return ($Result.stdout | ConvertFrom-Json -ErrorAction Stop)
    }
    catch {
        throw "$Context did not return valid JSON: $($_.Exception.Message)"
    }
}

Export-ModuleMember -Function @(
    'Get-UtcTimestamp',
    'Write-Utf8File',
    'Get-ObjectProperty',
    'Get-StringSha256',
    'Get-FileEvidence',
    'New-FileSnapshot',
    'ConvertTo-NativeArgument',
    'Invoke-CapturedProcess',
    'New-CommandEvidence',
    'Convert-CommandJson'
)
