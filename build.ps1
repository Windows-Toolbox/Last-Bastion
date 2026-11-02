# Baut das Spiel und den Karten-Editor im Release-Modus und legt sie als dist\LastBastion.exe und
# dist\LastBastionEditor.exe ab – das Spiel dort, wo die Windows-Toolbox es erwartet (DESIGN.md 12.7).
#
$ErrorActionPreference = 'Stop' # sorgt dafür, dass das Skript bei einem Fehler sofort abbricht.
# Wechselt in den Ordner, in dem das Skript liegt, egal von wo aus es gestartet wird. $PSScriptRoot ist leer, wenn die
# Zeilen eingefügt statt als Datei ausgeführt werden – dann bleibt es beim aktuellen Ordner (sonst ginge es ins Home).
if ($PSScriptRoot) { Set-Location $PSScriptRoot }
cargo build --release # baut beide Programme (src\main.rs und src\bin\editor.rs).
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } # hält an, wenn der Build fehlschlägt, damit keine alte Exe kopiert wird.

New-Item -ItemType Directory -Force dist | Out-Null # legt den Ordner dist an, falls er fehlt.
Copy-Item target\release\last_bastion.exe dist\LastBastion.exe -Force # kopiert das Spiel dorthin und benennt es dabei um.
Copy-Item target\release\editor.exe dist\LastBastionEditor.exe -Force # der Editor daneben – der Knopf „Map editor" im Spiel sucht ihn dort.
# Die Bilder (assets\) sind nicht in die Exe eingebaut – sie werden später zu groß. Erst löschen, damit in dist\ kein
# altes Bild liegen bleibt, das es im Projekt nicht mehr gibt; dann neu kopieren.
if (Test-Path dist\assets) { Remove-Item dist\assets -Recurse -Force }
Copy-Item assets dist\assets -Recurse
Write-Host "Fertig: dist\LastBastion.exe und dist\LastBastionEditor.exe"
