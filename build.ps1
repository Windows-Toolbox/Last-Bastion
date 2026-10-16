# Baut das Spiel im Release-Modus und legt es als dist\LastBastion.exe ab,
# wo die Windows-Toolbox es erwartet (DESIGN.md 12.7).
#
$ErrorActionPreference = 'Stop' # sorgt dafür, dass das Skript bei einem Fehler sofort abbricht.
Set-Location $PSScriptRoot # wechselt in den Ordner, in dem das Skript liegt, egal von wo aus es gestartet wird.

cargo build --release # baut das Programm.
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } # hält an, wenn der Build fehlschlägt, damit keine alte Exe kopiert wird.

New-Item -ItemType Directory -Force dist | Out-Null # legt den Ordner dist an, falls er fehlt.
Copy-Item target\release\last_bastion.exe dist\LastBastion.exe -Force # kopiert die Exe dorthin und benennt sie dabei um.
Write-Host "Fertig: dist\LastBastion.exe"
