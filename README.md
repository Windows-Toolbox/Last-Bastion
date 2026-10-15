# Last Bastion

Echtzeit-Strategie ums Überleben: Einheitensteuerung wie in **StarCraft**, Horden und Rundenaufbau wie in
**They Are Billions**, Produktionsketten und Stromnetz wie in **Factorio** – in Runden von etwa 30 Minuten.
*Last Bastion* ist ein Arbeitstitel.

Stand: **Konzept**. Das ganze Design steht in [DESIGN.md](DESIGN.md) (auf Englisch).

---

## Regeln für Claude (bitte zuerst lesen)

`CLAUDE.md` und Claudes Memory liegen nur lokal – deshalb stehen die Regeln hier:

- **Ich (Maximilian) schreibe den Code für das Spiel selbst.** Claude schreibt **keinen** Spielcode und liefert keine
  fertigen Lösungen, nur auf Anfrage.
- **Was Claude darf:** beraten, Konzepte erklären, das Design-Dokument mit mir weiterentwickeln, Fragen beantworten,
  Fehlermeldungen erklären, meinen Code reviewen, auf die passende Doku zeigen.
- **Erst einen Tipp, dann die Lösung.**
- **Ausnahme:** `DESIGN.md`, diese README und Build-/Einrichtungs-Skripte darf Claude pflegen.

---

## Dateien

| Datei | Was |
|---|---|
| `DESIGN.md` | Design-Dokument: Säulen, Ablauf einer Runde, Wirtschaft, Einheiten, Zombies, Karte, Technik, Fahrplan, offene Fragen |
| `dist\LastBastion.exe` | Das fertig gebaute Spiel (wird gebaut, nicht in git) – kommt mit dem ersten Prototyp |

## Anbindung an die Windows-Toolbox

Das Spiel ist ein eigenes Repo und weiß nichts von der [Windows-Toolbox](https://github.com/ItsMaxxx/windows-toolbox).
Die Toolbox hat nur eine kleine Seite im Tab „Games“, die diesen Ordner kennt (Knopf „Spielordner wählen“, sonst sucht
sie `last-bastion` neben dem Toolbox-Ordner). Damit das klappt, gilt (DESIGN.md, Abschnitt 12.7):

- Das fertige Spiel liegt immer unter **`dist\LastBastion.exe`** – egal mit welcher Sprache gebaut.
- Das Design-Dokument heißt `DESIGN.md` und liegt im Hauptordner.
- Die Einstellungen des Spielers liegen an einer festen Stelle (DESIGN.md, Abschnitt 11.4).

## Fahrplan

Die Prototyp-Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap) – davor die Liste
„Before step 1“ (Sprache, Koop, Speichern, wer baut, Wirtschaft in einer Tabelle durchrechnen …). Dann zuerst die
Einheitensteuerung, dann der Horden-Stresstest, nach Schritt 3 eine Mini-Runde: Macht das Spiel Spaß?

## Stand

- **2026-10-15:** Konzept als `DESIGN.md` angelegt (zuerst in der Windows-Toolbox unter `programs\last_bastion\`).
- **2026-10-15:** Einheiten überarbeitet (TAB-Infanterie, SC-Medic, Siege Tank, MG-Schütze); Blight heißt jetzt Creep;
  keine Bosse (nur Riesen wie in TAB); keine Förderbänder; Meta-Fortschritt wie in TAB ohne Kampagne; Endlos-Modus
  statt Kampagne/Daily Challenge; Sprache Rust oder C-Familie.
- **2026-10-15:** DESIGN.md ergänzt: „Before step 1“-Liste und Mini-Runde (neuer Schritt 4) im Fahrplan, Simulation
  in festen Takten, Entwickler-Werkzeuge, Einstellungen in einer JSON-Datei (11.4), neue offene Fragen 13–16.
  Hausstufe 1 heißt „Tent“ (Zelt).
- **2026-10-15:** Eigenes Repo, getrennt von der Windows-Toolbox. Die Toolbox startet `dist\LastBastion.exe`.
