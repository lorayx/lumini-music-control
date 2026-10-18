param([ValidateSet('check','test','bundle')][string]$Mode = 'bundle')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) { throw 'Install Microsoft C++ Build Tools with Desktop development with C++ and Windows SDK' }
$devCommand = Join-Path $installation 'Common7\Tools\VsDevCmd.bat'
$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $env:ComSpec
$start.Arguments = '/d /s /c ""' + $devCommand + '" -no_logo -arch=x64 -host_arch=x64 >nul && set"'
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$process = [Diagnostics.Process]::Start($start)
$environment = $process.StandardOutput.ReadToEnd() -split "`r?`n"
$process.WaitForExit()
if ($process.ExitCode -ne 0) { throw 'VsDevCmd failed' }
$process.Dispose()
foreach ($entry in $environment) {
    if ($entry -match '^(PATH|INCLUDE|LIB|LIBPATH|WindowsSdkDir|WindowsSDKVersion|VCToolsInstallDir|VCINSTALLDIR)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1],$matches[2],'Process') }
}
$sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10'
$sdkVersion = Get-ChildItem (Join-Path $sdkRoot 'Lib') -Directory | Sort-Object Name -Descending | Where-Object { Test-Path (Join-Path $_.FullName 'um\x64\kernel32.lib') } | Select-Object -First 1
if (-not $sdkVersion) { throw 'Windows SDK desktop libraries are missing' }
$env:LIB = (Join-Path $sdkVersion.FullName 'um\x64') + ';' + (Join-Path $sdkVersion.FullName 'ucrt\x64') + ';' + $env:LIB
$env:INCLUDE = (Join-Path $sdkRoot ('Include\' + $sdkVersion.Name + '\um')) + ';' + (Join-Path $sdkRoot ('Include\' + $sdkVersion.Name + '\shared')) + ';' + (Join-Path $sdkRoot ('Include\' + $sdkVersion.Name + '\ucrt')) + ';' + $env:INCLUDE
$env:PATH = (Join-Path $sdkRoot ('bin\' + $sdkVersion.Name + '\x64')) + ';' + $env:PATH
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = $cargoBin + ';' + $env:PATH
Push-Location $project
try {
    if ($Mode -eq 'bundle') { & npm.cmd run tauri -- build --target x86_64-pc-windows-msvc }
    elseif ($Mode -eq 'test') { & cargo.exe test --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc }
    else { & cargo.exe check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc }
    if ($LASTEXITCODE -ne 0) { throw "Build operation failed: $Mode" }
} finally { Pop-Location }
