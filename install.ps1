# Install ccl on Windows: download the latest release into ~/.ccl/bin and add
# that folder to your user PATH.
#
#   irm https://raw.githubusercontent.com/wangnguen/config-litellm/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "wangnguen/config-litellm"
$Asset = "ccl-x86_64-pc-windows-msvc.exe"
$BinDir = Join-Path $env:USERPROFILE ".ccl\bin"
$Exe = Join-Path $BinDir "ccl.exe"

# A plain release download rather than api.github.com: the API allows 60
# unauthenticated requests an hour per IP, which an office sharing one
# address runs out of.
$Url = "https://github.com/$Repo/releases/latest/download/$Asset"

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Write-Host "Downloading $Asset..." -ForegroundColor Cyan
$Tmp = "$Exe.new"
Invoke-WebRequest -Uri $Url -OutFile $Tmp -UseBasicParsing

# A running ccl.exe cannot be overwritten, but it can be renamed out of the way.
if (Test-Path $Exe) {
    $Old = Join-Path $BinDir "ccl.old.exe"
    if (Test-Path $Old) { Remove-Item $Old -Force -ErrorAction SilentlyContinue }
    Rename-Item $Exe "ccl.old.exe" -Force
}
Move-Item $Tmp $Exe -Force

# Edit the registry value rather than [Environment]::SetEnvironmentVariable:
# that API reads PATH back expanded, so writing it would freeze entries like
# %JAVA_HOME%\bin into whatever they point at today.
$Key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment", $true)
$Raw = [string]$Key.GetValue("Path", "", "DoNotExpandEnvironmentNames")
$Present = $Raw -split ";" | Where-Object { $_.TrimEnd("\") -ieq $BinDir }
if ($Present) {
    Write-Host "$BinDir is already in PATH." -ForegroundColor Yellow
} else {
    if ($Raw) {
        $Key.SetValue("Path", "$($Raw.TrimEnd(';'));$BinDir", $Key.GetValueKind("Path"))
    } else {
        $Key.SetValue("Path", $BinDir, [Microsoft.Win32.RegistryValueKind]::ExpandString)
    }
    # Touching any user variable through this API broadcasts the change, so
    # terminals opened from now on see the new PATH.
    [Environment]::SetEnvironmentVariable("CCL_PATH_REFRESH", $null, "User")
    Write-Host "Added $BinDir to your PATH." -ForegroundColor Green
}
$Key.Close()
$env:Path = "$env:Path;$BinDir"

Write-Host ""
Write-Host "Done: $Exe" -ForegroundColor Green
Write-Host "Open a project folder and run: ccl" -ForegroundColor Cyan
