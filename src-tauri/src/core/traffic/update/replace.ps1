param(
    [Parameter(Mandatory = $true)][string]$Plan,
    [switch]$Elevated
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$attemptRoot = [IO.Path]::GetFullPath((Split-Path -LiteralPath $Plan))
$statusFile = Join-Path $attemptRoot 'status.json'
$abortFile = Join-Path $attemptRoot 'abort'
$proceedFile = Join-Path $attemptRoot 'proceed'
$installed = [Collections.Generic.List[string]]::new()
$backedUp = [Collections.Generic.List[string]]::new()
$parentExited = $false
$config = $null

function Write-JsonFile([string]$Path, $Value) {
    $temporary = $Path + '.tmp'
    [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Write-Status([string]$State, [string]$Detail = '') {
    Write-JsonFile $statusFile @{ state = $State; detail = $Detail }
}

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}

function Assert-PlainPath([string]$Path) {
    $candidate = [IO.Path]::GetFullPath($Path)
    while ($candidate) {
        if (Test-Path -LiteralPath $candidate) {
            $item = Get-Item -LiteralPath $candidate -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "A replacement path contains a link or junction: $candidate"
            }
        }
        $parent = [IO.Path]::GetDirectoryName($candidate)
        if ($parent -eq $candidate) { break }
        $candidate = $parent
    }
}

function Assert-Member([string]$Name) {
    $parts = @($Name.Split('/'))
    foreach ($part in $parts) {
        if (-not $part -or $part.EndsWith('.') -or $part.EndsWith(' ') -or
            $part -match '[\\:\x00-\x1f<>"|?*]' -or $part -in @('.', '..') -or
            $part -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)') {
            throw "Unsafe package member: $Name"
        }
    }
    $rootFiles = @('clash-verge.exe', 'verge-mihomo.exe', 'verge-mihomo-alpha.exe', 'LICENSE', '使用说明.txt', 'release.json')
    if (($parts.Count -eq 1 -and $Name -cnotin $rootFiles) -or
        ($parts.Count -gt 1 -and ($parts.Count -ne 2 -or $parts[0] -cne 'resources'))) {
        throw "Unexpected package member: $Name"
    }
}

function Get-MemberPath([string]$Root, [string]$Name) {
    Assert-Member $Name
    $rootPrefix = [IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    $memberPath = [IO.Path]::GetFullPath((Join-Path $Root $Name))
    if (-not $memberPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'A package member escapes its directory.'
    }
    Assert-PlainPath $memberPath
    return $memberPath
}

function Read-Manifest([string]$Root, [string]$Version) {
    $manifestPath = Get-MemberPath $Root 'release.json'
    $manifest = [IO.File]::ReadAllText($manifestPath, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($manifest.appVersion -cne $Version -or $manifest.releaseTag -cne ('v' + $Version) -or $manifest.platform -cne 'windows-x64') {
        throw 'The portable manifest version or platform changed.'
    }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in $manifest.sha256.PSObject.Properties) {
        Assert-Member $entry.Name
        if ($entry.Name -eq 'release.json' -or -not $seen.Add($entry.Name) -or $entry.Value -notmatch '^[a-fA-F0-9]{64}$') {
            throw 'The portable manifest contains an invalid file entry.'
        }
    }
    return $manifest
}

function Start-Application {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        try {
            $shell = New-Object -ComObject Shell.Application
            $shell.ShellExecute($config.currentExecutable, '', $targetRoot, 'open', 1)
            return
        } catch { }
    }
    Start-Process -FilePath $config.currentExecutable -WorkingDirectory $targetRoot | Out-Null
}

function Write-Result([string]$State, [string]$Detail) {
    Write-JsonFile $config.resultFile @{
        status = $State; version = $config.version; detail = $Detail
        backupDirectory = $config.backupDirectory
    }
}

try {
    $config = [IO.File]::ReadAllText($Plan, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if (Test-Path -LiteralPath $abortFile) { exit 1 }
    $sourceRoot = [IO.Path]::GetFullPath($config.sourceDirectory)
    $targetRoot = [IO.Path]::GetFullPath((Split-Path -LiteralPath $config.currentExecutable))
    $workRoot = [IO.Path]::GetFullPath((Split-Path -LiteralPath $attemptRoot))
    $backupRoot = [IO.Path]::GetFullPath($config.backupDirectory)
    Assert-PlainPath $workRoot
    Assert-PlainPath $targetRoot
    Assert-PlainPath $backupRoot
    if ($sourceRoot -cne (Join-Path $workRoot 'package') -or
        -not (Split-Path -Leaf $workRoot).StartsWith('otool-update-') -or
        -not $backupRoot.StartsWith((Join-Path $targetRoot '.otool-backups') + '\', [StringComparison]::OrdinalIgnoreCase) -or
        $sourceRoot.StartsWith($targetRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $config.currentExecutable) -ine 'clash-verge.exe') {
        throw 'The replacement directories are not the reviewed portable locations.'
    }
    if (Test-Path -LiteralPath $backupRoot) { throw 'The backup directory already exists.' }
    $parentProcess = [Diagnostics.Process]::GetProcessById([int]$config.parentPid)
    if ($parentProcess.StartTime.ToUniversalTime() -gt [DateTime]::Parse($config.preparedAt).ToUniversalTime() -or
        [IO.Path]::GetFullPath($parentProcess.MainModule.FileName) -ine [IO.Path]::GetFullPath($config.currentExecutable)) {
        throw 'The running program changed after the update was prepared.'
    }
    $oldManifest = Read-Manifest $targetRoot $config.currentVersion
    $newManifest = Read-Manifest $sourceRoot $config.version
    if ((Get-Sha256 (Join-Path $sourceRoot 'release.json')) -ine $config.manifestSha256) {
        throw 'The downloaded manifest changed after confirmation.'
    }
    foreach ($entry in $newManifest.sha256.PSObject.Properties) {
        $sourceFile = Get-MemberPath $sourceRoot $entry.Name
        if ((Get-Sha256 $sourceFile) -ine $entry.Value) {
            throw ('The downloaded file changed: ' + $entry.Name)
        }
    }
    $newExecutable = Get-MemberPath $sourceRoot 'clash-verge.exe'
    if ((Get-Item -LiteralPath $newExecutable).VersionInfo.ProductVersion -cne $config.version) {
        throw 'The downloaded executable version does not match the release.'
    }
    $probe = Join-Path $targetRoot ('.otool-write-' + [Guid]::NewGuid().ToString('N'))
    try {
        $probeStream = [IO.File]::Open($probe, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $probeStream.Dispose()
        Remove-Item -LiteralPath $probe -Force
    } catch [UnauthorizedAccessException] {
        if ($Elevated) { throw }
        $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $PSCommandPath + '"'), '-Plan', ('"' + $Plan + '"'), '-Elevated')
        $administrator = Start-Process -FilePath $powershell -ArgumentList $arguments -Verb RunAs -WindowStyle Hidden -Wait -PassThru
        exit $administrator.ExitCode
    }
    if (Test-Path -LiteralPath $abortFile) { exit 1 }
    Write-Status 'ready'
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    while (-not $parentProcess.HasExited -or -not (Test-Path -LiteralPath $proceedFile)) {
        if (Test-Path -LiteralPath $abortFile) { exit 1 }
        if ([DateTime]::UtcNow -gt $deadline) { throw 'The program did not exit safely; no program files were replaced.' }
        Start-Sleep -Milliseconds 150
        $parentProcess.Refresh()
    }
    $parentExited = $true
    if (Test-Path -LiteralPath $abortFile) { exit 1 }
    $managed = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($oldManifest.sha256.PSObject.Properties) + @($newManifest.sha256.PSObject.Properties)) {
        [void]$managed.Add($entry.Name)
    }
    [void]$managed.Add('release.json')
    foreach ($name in $managed) {
        $targetFile = Get-MemberPath $targetRoot $name
        if (Test-Path -LiteralPath $targetFile) {
            $handle = [IO.File]::Open($targetFile, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
            $handle.Dispose()
        }
    }
    [IO.Directory]::CreateDirectory($backupRoot) | Out-Null
    foreach ($name in $managed) {
        $targetFile = Get-MemberPath $targetRoot $name
        if (Test-Path -LiteralPath $targetFile) {
            $backupFile = Get-MemberPath $backupRoot $name
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($backupFile)) | Out-Null
            Move-Item -LiteralPath $targetFile -Destination $backupFile
            $backedUp.Add($name)
        }
    }
    $newFiles = @($newManifest.sha256.PSObject.Properties.Name) + @('release.json')
    foreach ($name in $newFiles) {
        $targetFile = Get-MemberPath $targetRoot $name
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($targetFile)) | Out-Null
        $installed.Add($name)
        Copy-Item -LiteralPath (Get-MemberPath $sourceRoot $name) -Destination $targetFile
    }
    foreach ($entry in $newManifest.sha256.PSObject.Properties) {
        if ((Get-Sha256 (Get-MemberPath $targetRoot $entry.Name)) -ine $entry.Value) {
            throw ('The replacement file checksum does not match: ' + $entry.Name)
        }
    }
    Write-Result 'installed' 'Program files were replaced and a restart was requested.'
    Write-Status 'installed'
    Start-Application
    $workPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($workRoot.StartsWith($workPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        try { Remove-Item -LiteralPath $workRoot -Recurse -Force } catch { }
    }
} catch {
    $detail = $_.Exception.Message
    $rollbackErrors = [Collections.Generic.List[string]]::new()
    foreach ($name in $installed) {
        try { Remove-Item -LiteralPath (Get-MemberPath $targetRoot $name) -Force -ErrorAction Stop }
        catch { $rollbackErrors.Add($_.Exception.Message) }
    }
    foreach ($name in $backedUp) {
        try { Move-Item -LiteralPath (Get-MemberPath $backupRoot $name) -Destination (Get-MemberPath $targetRoot $name) -Force }
        catch { $rollbackErrors.Add($_.Exception.Message) }
    }
    if ($rollbackErrors.Count) { $detail += ' Rollback needs attention: ' + ($rollbackErrors -join '; ') }
    try { Write-Status 'error' $detail } catch { }
    if ($config) {
        try { Write-Result 'error' $detail } catch { }
    }
    if ($parentExited -and -not $rollbackErrors.Count) {
        try { Start-Application } catch { }
    }
    exit 1
}
