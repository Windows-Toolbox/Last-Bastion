# Baut das Spiel und den Karten-Editor im Release-Modus und legt sie als dist\LastBastion.exe und
# dist\LastBastionEditor.exe ab – das Spiel dort, wo die Windows-Toolbox es erwartet (DESIGN.md 12.7).
#
$ErrorActionPreference = 'Stop' # sorgt dafür, dass das Skript bei einem Fehler sofort abbricht.
Set-Location $PSScriptRoot # wechselt in den Ordner, in dem das Skript liegt, egal von wo aus es gestartet wird.

cargo build --release # baut beide Programme (src\main.rs und src\bin\editor.rs).
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } # hält an, wenn der Build fehlschlägt, damit keine alte Exe kopiert wird.

New-Item -ItemType Directory -Force dist | Out-Null # legt den Ordner dist an, falls er fehlt.
Copy-Item target\release\last_bastion.exe dist\LastBastion.exe -Force # kopiert das Spiel dorthin und benennt es dabei um.
Copy-Item target\release\editor.exe dist\LastBastionEditor.exe -Force # der Editor daneben – der Knopf „Map editor" im Spiel sucht ihn dort.
Write-Host "Fertig: dist\LastBastion.exe und dist\LastBastionEditor.exe"
