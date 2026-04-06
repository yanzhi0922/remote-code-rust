[CmdletBinding()]
param(
    [string]$SourceRoot = "C:\Users\Yanzh\Desktop\remote-code",
    [string]$OutputRoot = "fixtures/reference/legacy-runtime-src"
)

$ErrorActionPreference = "Stop"

$python = Get-Command py -ErrorAction SilentlyContinue
if ($python) {
    & $python.Source -3 "$PSScriptRoot\collect_reference_fixtures.py" --source-root $SourceRoot --output-root $OutputRoot
    exit $LASTEXITCODE
}

$python = Get-Command python -ErrorAction SilentlyContinue
if ($python) {
    & $python.Source "$PSScriptRoot\collect_reference_fixtures.py" --source-root $SourceRoot --output-root $OutputRoot
    exit $LASTEXITCODE
}

throw "Python 3 is required to run scripts/collect_reference_fixtures.py"
