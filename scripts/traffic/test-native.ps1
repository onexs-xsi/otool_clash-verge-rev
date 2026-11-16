param([string]$Filter = 'traffic')

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location -LiteralPath $projectRoot
try {
    $messages = & cargo test -p clash-verge --lib --no-run --message-format=json
    if ($LASTEXITCODE -ne 0) {
        $messages | Where-Object { $_.StartsWith('{') } | ForEach-Object {
            $entry = $_ | ConvertFrom-Json
            if ($entry.reason -eq 'compiler-message') { Write-Output $entry.message.rendered }
        }
        throw 'Native traffic tests did not compile.'
    }
    $artifact = $messages | Where-Object { $_.StartsWith('{') } | ForEach-Object { $_ | ConvertFrom-Json } |
        Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'app_lib' -and $_.executable } |
        Select-Object -Last 1
    if (-not $artifact) { throw 'Cargo did not report the app_lib test executable.' }
    $sdk = Get-ChildItem -Path 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\mt.exe' -File |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $sdk) { throw 'The Windows SDK manifest tool (mt.exe) is required.' }
    $testExe = (Resolve-Path -LiteralPath $artifact.executable).Path
    $targetRoot = (Resolve-Path -LiteralPath (Join-Path $projectRoot 'target')).Path + [IO.Path]::DirectorySeparatorChar
    if (-not $testExe.StartsWith($targetRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Cargo test executable is outside target/.' }
    & $sdk.FullName -nologo -manifest (Join-Path $PSScriptRoot 'windows-test.manifest') "-outputresource:$testExe;#1"
    if ($LASTEXITCODE -ne 0) { throw 'Could not apply the Windows test manifest.' }
    & $testExe $Filter --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Native traffic tests failed.' }
    if ($env:TRAFFIC_ICON_TEST_PATH) {
        & $testExe installed_package_recovers_its_icon --ignored --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'Installed package icon test failed.' }
    }
    if ($env:TRAFFIC_UPDATE_TEST_ARCHIVE) {
        & $testExe published_portable_archive_passes_native_validation --ignored --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'The published portable archive failed native update validation.' }
    }
} finally {
    Pop-Location
}
