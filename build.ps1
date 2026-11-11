# Baut das Spiel und den Karten-Editor im Release-Modus und legt sie als dist\LastBastion.exe und
# dist\LastBastionEditor.exe ab – das Spiel dort, wo die Windows-Toolbox es erwartet (DESIGN.md 12.7).
# Läuft das Spiel oder der Editor gerade, klappt das trotzdem: die laufende Version wird beiseitegeschoben (siehe unten).
#
$ErrorActionPreference = 'Stop' # sorgt dafür, dass das Skript bei einem Fehler sofort abbricht.
# Wechselt in den Ordner, in dem das Skript liegt, egal von wo aus es gestartet wird. $PSScriptRoot ist leer, wenn die
# Zeilen eingefügt statt als Datei ausgeführt werden – dann bleibt es beim aktuellen Ordner (sonst ginge es ins Home).
if ($PSScriptRoot) { Set-Location $PSScriptRoot }
cargo build --release # baut beide Programme (src\main.rs und src\bin\editor.rs).
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } # hält an, wenn der Build fehlschlägt, damit keine alte Exe kopiert wird.

New-Item -ItemType Directory -Force dist | Out-Null # legt den Ordner dist an, falls er fehlt.

# Windows sperrt eine laufende Exe: überschreiben oder löschen geht nicht, umbenennen aber schon. Also wird die alte
# zu „LastBastion.<Zeit>.old.exe“ – sie läuft einfach weiter – und die neue kommt an ihren Platz. Beim nächsten Start
# läuft die neue. Alte .old.exe werden weggeräumt, sobald sie nicht mehr laufen (die noch laufenden bleiben liegen).
Get-ChildItem dist\*.old.exe -ErrorAction SilentlyContinue | ForEach-Object {
    Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
}
function Install-Exe($from, $to) {
    $target = Join-Path dist $to
    if (Test-Path $target) {
        try {
            Copy-Item $from $target -Force # der Normalfall: das Programm läuft nicht
            return
        } catch {
            $old = [IO.Path]::GetFileNameWithoutExtension($to) + '.' + (Get-Date -Format 'HHmmss') + '.old.exe'
            Rename-Item $target $old # läuft gerade: beiseiteschieben
            Write-Host "$to läuft gerade - die laufende Version heißt jetzt $old, die neue startet beim nächsten Mal."
        }
    }
    Copy-Item $from $target -Force
}
Install-Exe target\release\last_bastion.exe LastBastion.exe # das Spiel, dabei umbenannt
Install-Exe target\release\editor.exe LastBastionEditor.exe # der Editor daneben – der Knopf „Map editor" im Spiel sucht ihn dort.

# Die Bilder (assets\) sind nicht in die Exe eingebaut – sie werden später zu groß. robocopy /MIR spiegelt den Ordner:
# kopiert nur, was sich geändert hat, und löscht in dist\ Bilder, die es im Projekt nicht mehr gibt. Anders als
# „alles löschen, neu kopieren“ fehlen die Bilder dabei keinen Moment – wichtig, wenn das Spiel gerade läuft.
robocopy assets dist\assets /MIR /NJH /NJS /NFL /NDL /NP | Out-Null
if ($LASTEXITCODE -ge 8) { Write-Host "Bilder kopieren fehlgeschlagen (robocopy $LASTEXITCODE)"; exit $LASTEXITCODE }
$global:LASTEXITCODE = 0 # robocopy meldet 1–7 auch bei Erfolg („es wurde etwas kopiert“) – das ist kein Fehler.
Write-Host "Fertig: dist\LastBastion.exe und dist\LastBastionEditor.exe"
