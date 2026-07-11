# BitShit Windows installer entry point.
# Accepts PowerShell-style, GNU-style, and positional backend syntax:
#   .\install.ps1 -Backend cuda
#   .\install.ps1 --backend cuda
#   .\install.ps1 cuda

$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$Backend = 'auto'
$Yes = $false
$NoLegacyAlias = $false
$NoMigrate = $false

for ($i = 0; $i -lt $args.Count; $i++) {
    $arg = [string]$args[$i]
    switch -Regex ($arg) {
        '^(?i)--?backend$' {
            if ($i + 1 -ge $args.Count) {
                throw '[bitshit] --backend requires one of: auto, cpu, cuda'
            }
            $i++
            $Backend = ([string]$args[$i]).ToLowerInvariant()
            continue
        }
        '^(?i)--?backend=(auto|cpu|cuda)$' {
            $Backend = $Matches[1].ToLowerInvariant()
            continue
        }
        '^(?i)--?yes$' {
            $Yes = $true
            continue
        }
        '^(?i)--?no-legacy-alias$' {
            $NoLegacyAlias = $true
            continue
        }
        '^(?i)--?no-migrate$' {
            $NoMigrate = $true
            continue
        }
        '^(?i)(auto|cpu|cuda)$' {
            $Backend = $Matches[1].ToLowerInvariant()
            continue
        }
        default {
            throw "[bitshit] Unknown installer argument: $arg"
        }
    }
}

if ($Backend -notin @('auto', 'cpu', 'cuda')) {
    throw "[bitshit] Invalid backend '$Backend'. Expected auto, cpu, or cuda."
}

$Setup = Join-Path $PSScriptRoot 'scripts\setup-windows.ps1'
if (-not (Test-Path -LiteralPath $Setup)) {
    throw "[bitshit] Missing Windows setup script: $Setup"
}

# Hashtable splatting preserves named PowerShell parameters. Array splatting would
# pass '-Backend' as the first positional value, which ValidateSet then rejects.
$Forward = @{
    Backend = $Backend
}
if ($Yes) { $Forward.Yes = $true }
if ($NoLegacyAlias) { $Forward.NoLegacyAlias = $true }
if ($NoMigrate) { $Forward.NoMigrate = $true }

& $Setup @Forward
if ($LASTEXITCODE -ne 0) {
    throw "[bitshit] Windows setup failed with exit code $LASTEXITCODE."
}
