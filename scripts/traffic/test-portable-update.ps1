$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
$fixtureRoot = Join-Path $temporaryRoot ('otool-portable-test-' + [Guid]::NewGuid().ToString('N'))
$ownedDirectories = [Collections.Generic.List[string]]::new()
$ownedDirectories.Add($fixtureRoot)
[IO.Directory]::CreateDirectory($fixtureRoot) | Out-Null
$encoding = [Text.UTF8Encoding]::new($false)
$compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$sourceFile = Join-Path $fixtureRoot 'Fixture.cs'
$fixtureSource = @'
using System;
using System.IO;
using System.Threading;
using System.Reflection;
#if OLD
[assembly: AssemblyInformationalVersion("2.6.0-otool.14")]
#else
[assembly: AssemblyInformationalVersion("2.6.0-otool.15")]
#endif
class Fixture {
    static void Main(string[] args) {
        if (args.Length > 0 && args[0] == "--wait") {
            File.WriteAllText("parent-ready", "ready");
            while (!File.Exists("quit")) Thread.Sleep(50);
        } else {
            File.WriteAllText("started.txt", Assembly.GetExecutingAssembly().GetCustomAttribute<AssemblyInformationalVersionAttribute>().InformationalVersion);
        }
    }
}
'@

function Assert-True($Value, [string]$Message) {
    if (-not $Value) { throw $Message }
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 8), $encoding)
}

function Wait-Until([scriptblock]$Condition, [string]$Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not (& $Condition)) {
        if ([DateTime]::UtcNow -gt $deadline) { throw $Message }
        Start-Sleep -Milliseconds 80
    }
}

function New-Portable([string]$Root, [string]$Version, [string]$Executable) {
    [IO.Directory]::CreateDirectory((Join-Path $Root 'resources')) | Out-Null
    Copy-Item -LiteralPath $Executable -Destination (Join-Path $Root 'clash-verge.exe')
    $names = @('verge-mihomo.exe', 'verge-mihomo-alpha.exe', 'LICENSE', '使用说明.txt', 'resources/clash-verge-service.exe', 'resources/clash-verge-service-install.exe', 'resources/clash-verge-service-uninstall.exe')
    foreach ($name in $names) { [IO.File]::WriteAllText((Join-Path $Root $name), ($Version + ':' + $name), $encoding) }
    $hashes = [ordered]@{}
    foreach ($name in @('clash-verge.exe') + $names) { $hashes[$name] = (Get-FileHash -LiteralPath (Join-Path $Root $name) -Algorithm SHA256).Hash.ToLowerInvariant() }
    Write-Json (Join-Path $Root 'release.json') @{appVersion=$Version;releaseTag=('v' + $Version);platform='windows-x64';sha256=$hashes}
}

try {
    [IO.File]::WriteAllText($sourceFile, $fixtureSource, $encoding)
    $oldBinary = Join-Path $fixtureRoot 'old.exe'
    $newBinary = Join-Path $fixtureRoot 'new.exe'
    & $compiler /nologo /target:winexe /platform:x64 /define:OLD "/out:$oldBinary" $sourceFile
    if ($LASTEXITCODE -ne 0) { throw 'Old fixture did not compile.' }
    & $compiler /nologo /target:winexe /platform:x64 "/out:$newBinary" $sourceFile
    if ($LASTEXITCODE -ne 0) { throw 'New fixture did not compile.' }
    foreach ($scenario in @('replace', 'abort', 'tampered', 'locked', 'rollback')) {
        $workRoot = Join-Path $temporaryRoot ('otool-update-' + [Guid]::NewGuid().ToString('N'))
        $ownedDirectories.Add($workRoot)
        $sourceRoot = Join-Path $workRoot 'package'
        $attempt = Join-Path $workRoot 'install-fixture'
        $targetRoot = Join-Path $fixtureRoot ($scenario + " portable 中文 ' folder")
        $resultFile = Join-Path $fixtureRoot ($scenario + '-result.json')
        [IO.Directory]::CreateDirectory($attempt) | Out-Null
        New-Portable $targetRoot '2.6.0-otool.14' $oldBinary
        New-Portable $sourceRoot '2.6.0-otool.15' $newBinary
        [IO.File]::WriteAllText((Join-Path $targetRoot 'keep-me.txt'), 'unmanaged user file', $encoding)
        $oldHash = (Get-FileHash -LiteralPath (Join-Path $targetRoot 'clash-verge.exe') -Algorithm SHA256).Hash
        $parent = Start-Process -FilePath (Join-Path $targetRoot 'clash-verge.exe') -ArgumentList '--wait' -WorkingDirectory $targetRoot -WindowStyle Hidden -PassThru
        $helper = $null
        $fileLock = $null
        try {
            Wait-Until { Test-Path -LiteralPath (Join-Path $targetRoot 'parent-ready') } 'Fixture parent did not start.'
            $backup = Join-Path $targetRoot '.otool-backups/fixture'
            $plan = Join-Path $attempt 'plan.json'
            Write-Json $plan @{
                version='2.6.0-otool.15'; currentVersion='2.6.0-otool.14'
                currentExecutable=(Join-Path $targetRoot 'clash-verge.exe'); sourceDirectory=$sourceRoot
                backupDirectory=$backup; parentPid=$parent.Id; preparedAt=[DateTime]::UtcNow.ToString('o')
                resultFile=$resultFile; manifestSha256=(Get-FileHash -LiteralPath (Join-Path $sourceRoot 'release.json') -Algorithm SHA256).Hash
            }
            $script = Join-Path $attempt 'replace.ps1'
            $scriptBytes = [IO.File]::ReadAllBytes((Join-Path $projectRoot 'src-tauri/src/core/traffic/update/replace.ps1'))
            [IO.File]::WriteAllBytes($script, ([byte[]]@(0xef,0xbb,0xbf) + $scriptBytes))
            if ($scenario -eq 'tampered') { [IO.File]::AppendAllText((Join-Path $sourceRoot 'LICENSE'), 'tampered') }
            $arguments = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"' + $script + '"'),'-Plan',('"' + $plan + '"'))
            $helper = Start-Process -FilePath $powershell -ArgumentList $arguments -WindowStyle Hidden -PassThru
            $statusPath = Join-Path $attempt 'status.json'
            Wait-Until { Test-Path -LiteralPath $statusPath } 'Replacement helper did not report readiness.'
            $status = [IO.File]::ReadAllText($statusPath) | ConvertFrom-Json
            Assert-True ((Get-FileHash -LiteralPath (Join-Path $targetRoot 'clash-verge.exe')).Hash -eq $oldHash) 'The running executable changed before safe exit.'
            if ($scenario -eq 'tampered') {
                Assert-True ($status.state -eq 'error') 'Tampered download reached readiness.'
                Assert-True (-not $parent.HasExited) 'Tampered download closed the running program.'
            } elseif ($scenario -eq 'abort') {
                Assert-True ($status.state -eq 'ready') ('Unexpected status: ' + $status.detail)
                [IO.File]::WriteAllText((Join-Path $attempt 'abort'), 'abort')
                Assert-True ($helper.WaitForExit(15000)) 'Aborted helper did not exit.'
                Assert-True (-not $parent.HasExited) 'Cancellation closed the running program.'
            } else {
                Assert-True ($status.state -eq 'ready') ('Unexpected status: ' + $status.detail)
                if ($scenario -eq 'locked') { $fileLock = [IO.File]::Open((Join-Path $targetRoot 'LICENSE'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read) }
                if ($scenario -eq 'rollback') { $fileLock = [IO.File]::Open((Join-Path $sourceRoot 'resources/clash-verge-service.exe'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None) }
                [IO.File]::WriteAllText((Join-Path $attempt 'proceed'), 'approved')
                [IO.File]::WriteAllText((Join-Path $targetRoot 'quit'), 'quit')
                Assert-True ($parent.WaitForExit(15000)) 'Fixture parent did not exit normally.'
                Assert-True ($helper.WaitForExit(20000)) 'Replacement helper did not exit.'
                if ($fileLock) { $fileLock.Dispose(); $fileLock = $null }
                $result = [IO.File]::ReadAllText($resultFile) | ConvertFrom-Json
                if ($scenario -eq 'replace') {
                    Assert-True ($result.status -eq 'installed') ('Replacement failed: ' + $result.detail)
                    Assert-True ((Get-FileHash -LiteralPath (Join-Path $backup 'clash-verge.exe')).Hash -eq $oldHash) 'The previous executable was not backed up.'
                    Wait-Until { Test-Path -LiteralPath (Join-Path $targetRoot 'started.txt') } 'The replacement program was not restarted.'
                    Assert-True ([IO.File]::ReadAllText((Join-Path $targetRoot 'started.txt')) -eq '2.6.0-otool.15') 'The wrong program restarted.'
                } else {
                    Assert-True ($result.status -eq 'error') 'The injected file failure was not reported.'
                    $manifest = [IO.File]::ReadAllText((Join-Path $targetRoot 'release.json')) | ConvertFrom-Json
                    Assert-True ($manifest.appVersion -eq '2.6.0-otool.14') 'Rollback did not restore the manifest.'
                    foreach ($entry in $manifest.sha256.PSObject.Properties) { Assert-True ((Get-FileHash -LiteralPath (Join-Path $targetRoot $entry.Name)).Hash -ieq $entry.Value) ('Old file was not preserved: ' + $entry.Name) }
                }
            }
            Assert-True ([IO.File]::ReadAllText((Join-Path $targetRoot 'keep-me.txt')) -eq 'unmanaged user file') 'An unmanaged user file was changed.'
            Write-Output ('PASS: ' + $scenario)
        } finally {
            if ($fileLock) { $fileLock.Dispose() }
            if (-not $parent.HasExited) {
                [IO.File]::WriteAllText((Join-Path $targetRoot 'quit'), 'quit')
                [void]$parent.WaitForExit(15000)
            }
            if ($helper -and -not $helper.HasExited) {
                [IO.File]::WriteAllText((Join-Path $attempt 'abort'), 'abort')
                [void]$helper.WaitForExit(15000)
            }
        }
    }
} finally {
    foreach ($ownedDirectory in $ownedDirectories) {
        $resolvedOwned = [IO.Path]::GetFullPath($ownedDirectory)
        if (-not $resolvedOwned.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf $resolvedOwned) -notmatch '^otool-(?:portable-test|update)-[a-f0-9]{32}$') { throw 'Refusing cleanup outside owned fixture directories.' }
        if (Test-Path -LiteralPath $resolvedOwned) { Remove-Item -LiteralPath $resolvedOwned -Recurse -Force }
    }
}
