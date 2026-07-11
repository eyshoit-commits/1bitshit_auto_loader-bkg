param(
    [Parameter(Position = 0)]
    [string]$Backend = 'auto',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$AllowedBackends = @('auto','cpu','cuda')
if ($Backend -eq '-Backend' -or $Backend -eq '--backend') {
    if (-not $RemainingArgs -or $RemainingArgs.Count -lt 1) {
        throw 'FEHLER: Nach -Backend fehlt der Wert auto, cpu oder cuda.'
    }
    $Backend = $RemainingArgs[0]
}
$Backend = $Backend.Trim().ToLowerInvariant()
if ($AllowedBackends -notcontains $Backend) {
    throw "FEHLER: Ungültiges Backend '$Backend'. Erlaubt sind auto, cpu oder cuda."
}

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

function Say([string]$Message) { Write-Host $Message }
function Fail([string]$Message) { throw "FEHLER: $Message" }

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Fail 'git wurde nicht gefunden.'
}

Set-Location $RepoRoot
Say 'BitShit Update startet.'
Say 'Hole den aktuellen Stand von GitHub.'

git fetch origin --prune
if ($LASTEXITCODE -ne 0) { Fail 'Git fetch ist fehlgeschlagen.' }

$CurrentBranch = (git branch --show-current).Trim()
if ([string]::IsNullOrWhiteSpace($CurrentBranch)) {
    git switch main
    if ($LASTEXITCODE -ne 0) { Fail 'Wechsel auf main ist fehlgeschlagen.' }
} elseif ($CurrentBranch -ne 'main') {
    Say "Wechsle von Branch $CurrentBranch auf main."
    git switch main
    if ($LASTEXITCODE -ne 0) { Fail 'Wechsel auf main ist fehlgeschlagen.' }
}

git reset --hard origin/main
if ($LASTEXITCODE -ne 0) { Fail 'Aktualisierung auf origin/main ist fehlgeschlagen.' }

$Installer = Join-Path $RepoRoot 'scripts\setup-windows.ps1'
if (-not (Test-Path $Installer)) {
    Fail 'scripts\setup-windows.ps1 fehlt im Repository.'
}

Say "Starte Aktualisierung und Neuinstallation über MSYS2 UCRT64 mit Backend $Backend."
& $Installer -Backend $Backend -Yes
if ($LASTEXITCODE -ne 0) {
    Fail "Installation ist mit Exitcode $LASTEXITCODE fehlgeschlagen."
}

Say 'BitShit Update abgeschlossen.'
