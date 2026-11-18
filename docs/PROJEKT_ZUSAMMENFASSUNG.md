# Klassenwerk: Projektzusammenfassung

Stand: Version 1.1.0 (18.11.2026). Diese Seite ist die Grundlage für die Weiterentwicklung: was das Programm kann, wie es aufgebaut ist, wie man es baut und was als Nächstes kommt.

## 1. Überblick

Klassenwerk ist eine Aufgabendatenbank, ein Klausurplaner und ein Notenbuch für Lehrkräfte. Es ist ein moderner Neubau des Windows-Programms **KlasseDozent 4.8**; Aufgaben-Exporte von KlasseDozent (XML) lassen sich importieren und wieder exportieren. Notenschlüssel, Druck und Standardwerte folgen eigenen Regeln (`docs/REGELN.md`).

| | |
|---|---|
| Technik | Rust (Edition 2021) mit egui/eframe 0.31; Paket `klassenwerk`, Programm `Klassenwerk` |
| Plattformen | Windows, Linux (x86_64), macOS (Apple Silicon und Intel) |
| Umfang | ca. 20 000 Zeilen Rust in 26 Dateien, 64 Unit-Tests |
| Lizenz | gemeinfrei, CC0 1.0 (`LICENSE`); mit Claude (Anthropic) entwickelt, kein Urheberanspruch. Fremdpakete: `THIRD_PARTY_LICENSES.txt` |
| Repository | https://github.com/buchhems/klassenwerk |

## 2. Funktionen

### Aufgaben
- **Aufgabenpool** mit Musterlösungen: Fächerbaum (Fach, Schlagwort 1/2), Volltextsuche, Filter, Favoriten, Verwendungshistorie, Schwierigkeit, Tags, interne Bemerkungen
- **Teilaufgaben** (a, b, c) mit eigenen Punkten und erwarteter Leistung (Erwartungshorizont); die Aufgabe hat dann deren Summe. Teilaufgaben im Aufgabentext („a) … (2 P.)“, „| a) | … | 2 P. |“) werden erkannt; die Punkte lassen sich in den Text zurückschreiben.
- **Zufallszahlen (Aufgabenvarianten):** Variablen mit Bereich und Schrittweite; `{=a}`, `{=2*a+b}`, `{=a/3;2}` in Text, Lösung, Erwartung und Formeln. Jede Mappe (und jede Gruppe) hat eigene Zahlen.
- **Rechtschreibprüfung** im Aufgabeneditor mit einem installierten Hunspell-Wörterbuch (LibreOffice, Linux) oder einer gewählten .dic-Datei; Vorschläge und eigenes Wörterbuch per Rechtsklick
- **Thesaurus:** Synonyme per Rechtsklick auf ein Wort, aus dem OpenThesaurus von LibreOffice/Linux (`th_de_DE_v2.dat`) oder einer gewählten Datei
- **Editor:** fett, kursiv, unterstrichen, hoch-/tiefgestellt, Farben, Aufzählungen, Überschriften, Symbole, Kästchen, Antwortlinien, Links
- **Tabellen** mit und ohne Rahmen; Menü „▦ Tabelle“: Zeilen und Spalten einfügen und löschen, Zellen nach rechts oder unten verbinden und wieder teilen, Rahmen je Zelle
- **Schrift je Aufgabe:** eigene Schriftart und -größe für den Druck (z. B. größer für eine Förderklasse); sonst gilt die Standardschrift
- **Bilder** aus Datei, aus der Zwischenablage (Button oder Alt+V) oder per Drag & Drop
- **Formeleditor** mit Live-Vorschau: Brüche, Wurzeln, Potenzen, Indizes, Integrale, Summen, Grenzwerte, Vektorpfeile, griechische Buchstaben, Matrizen und Vektoren, Fallunterscheidungen, mehrzeilige Rechnungen mit ausgerichteten Gleichheitszeichen. Formeln lassen sich auch direkt im Text schreiben (`$x^2$`). Formeln aus dem alten Formeleditor von KlasseDozent werden weiter angezeigt und gedruckt.
- Zum Start 25 Beispielaufgaben (Mathematik, Physik, Chemie, Wirtschaft, Deutsch, Englisch), eigens für Klassenwerk geschrieben und wie das Programm gemeinfrei; sie zeigen Formeln, Teilaufgaben, Erwartungshorizont und Zufallszahlen

### Klausuren und Übungen
- Klausuren, Übungen, Arbeitsblätter und Skripte; Typen frei definierbar
- Zusatztexte, Zufallsauffüllen aus dem Pool, Archiv, Vermerk „durchgeführt“
- **Erwartungshorizont:** Teilaufgaben mit erwarteter Leistung und Punkten, Musterlösungen und Notenschlüssel auf einem Blatt
- **Neue Zahlen würfeln** für Aufgaben mit Zufallszahlen; Druck, Lösung und Erwartungshorizont einer Mappe verwenden dieselben Zahlen
- **Parallelklausur Gruppe A/B:** Ersatzaufgaben mit gleichem Fach und Thema und ähnlicher Punktzahl, wahlweise gemischte Reihenfolge

### Druck und Export
- Druckvorlagen mit Platzhaltern (`{Titel}`, `{Gruppe}`, `{Gesamtpunkte}` …), Schullogo, Notenschlüssel-Tabelle (mit Prozentwerten und Zeile „Anzahl“ auf Wunsch), Notenspiegel mit Durchschnitt und Balkendiagramm
- Ausgabe als HTML/PDF (über den Browser), **Word (DOCX)** mit bearbeitbaren Word-Formeln, RTF
- Aufgaben im XML-Format von KlasseDozent importieren und exportieren; Aufgabenpakete `.kwerk`
- **Import aus Word und RTF:** ein Dokument wird an Markierungen („Aufgabe=1“ / „A#1“, „Lösung=1“ / „L#1“, dazu Fach, Schlagwörter, Punkte, Leerzeilen wie bei KDWord) in Aufgaben geteilt; Formatierung, Listen, Tabellen, Bilder und Word-Formeln werden übernommen, Punkte aus Klammern der Lösung addiert. Vorschau mit Prüfung vor dem Übernehmen. Ohne Markierungen wird das ganze Dokument eine Aufgabe.
- **Moodle:** Aufgaben oder eine ganze Klausur als Moodle-XML (Freitextfragen mit Punkten, Musterlösung als Feedback, Erwartungshorizont als Bewertungshinweis, Bilder eingebettet, Formeln als TeX)
- **Austauschordner:** Aufgaben mit Kolleginnen und Kollegen über einen gemeinsamen Ordner (Netzlaufwerk, Cloud, USB-Stick) teilen; beim Holen werden nur fehlende Aufgaben übernommen

### Noten
- **Notenschlüssel** IHK (Schlüssel der IHKs) und Gymnasium (Zentralabitur NRW), dazu „Noten mit Tendenz (1+ bis 6)“ und eigene Schlüssel; nötige Punkte werden auf halbe, ganze oder Zehntelpunkte aufgerundet. Gedruckte Grenze und berechnete Note stimmen immer überein.
- **Ergebnisse:** Punkte je Schüler oder je Aufgabe/Teilaufgabe (mit Durchschnitt je Aufgabe), „fehlt“; Einlesen aus CSV (Excel) mit passender Vorlage
- **Klassen & Notenbuch:** Klassenlisten (aus Excel einfügen oder aus CSV-/Textdatei), Klausuren einer Klasse zuordnen, mündliche und sonstige Leistungen als eigene Spalten, Gewichtung, Durchschnitte getrennt nach schriftlich und sonstig, Gesamtnote mit einstellbarem Anteil, Vorschlag für die Zeugnisnote (überschreibbar), Ansicht nach Schuljahr oder Halbjahr mit Zeugnisnote je Halbjahr, Notenentwicklung je Schüler als Diagramm, CSV-Export und Druck

### Daten und Bedienung
- Automatisches Speichern mit 5 Sicherungsgenerationen, manuelle Sicherung; aus einer Sicherung lassen sich einzelne Aufgaben, Klausuren und Klassen zurückholen (fehlende und geänderte werden markiert) oder alles wiederherstellen
- Fächer und Schlagwörter global umbenennen
- Heller und dunkler Modus
- **Updates:** Beim Start (höchstens einmal am Tag, abschaltbar) fragt Klassenwerk bei GitHub nach einer neuen Version. Unter Windows (Installer) lädt es sie herunter, installiert sie und startet neu; portable Version, macOS und Linux bekommen das Paket in den Ordner Downloads.

### KI-Assistent (MCP)
- `Klassenwerk --mcp` startet einen MCP-Server. Eingerichtet wird er in den Einstellungen unter „KI-Assistent“ (Claude Desktop inkl. Store-Version, Claude Code).
- Werkzeuge: `uebersicht`, `aufgaben_suchen`, `aufgabe_lesen`, `klausuren_auflisten`, `klausur_lesen`, `notenschluessel_berechnen`, `aufgabe_anlegen` (auch mit Teilaufgaben, Erwartungshorizont und Zufallszahlen), `klausur_entwurf_anlegen`
- Der Assistent darf lesen und neu anlegen, aber nichts ändern oder löschen. **Schülerdaten sind nicht zugänglich**: keine Klassen, Namen, Ergebnisse (auch nicht je Teilaufgabe), mündliche Noten, Zeugnisnoten, Notenbuch oder interne Bemerkungen. Der Test `mcp_never_shows_student_data` sichert das ab.
- Formeln schreibt der Assistent als `$LaTeX$`. Seine Einträge tragen ✦ und IDs ab 1 000 000 000. Die laufende App übernimmt sie automatisch, mit einer Prüfung alle 2 s und vor jedem Speichern.

## 3. Aufbau

### Ordner

| Pfad | Inhalt |
|---|---|
| `app/src/` | Quellcode (siehe unten) |
| `app/assets/` | Programmsymbol: Vorlage `icon.png` (1024 px), daraus `icon-256.png` (Fenster unter macOS/Linux) und `logo-64.png` (kleine Form nur mit „K“: Logo in der App, Fenster unter Windows, `.ico` bis 32 px); `windows/`: `.ico`, `.rc` und daraus erzeugte `.res`/`.obj` für Symbol und Versionsangaben der `.exe` |
| `app/samples/` | `beispielaufgaben.json`: die Beispielaufgaben (Format wie beim MCP-Werkzeug `aufgabe_anlegen`, Formeln als `$…$`), werden beim ersten Start geladen |
| `packaging/make-icons.py` | erzeugt `icon-256.png`, `logo-64.png` und `klassenwerk.ico` aus `icon.png` (danach `make-resources.sh`) |
| `packaging/windows/` | `build-windows.ps1` (Programm, Zip, Installer), `klassenwerk.iss` (Inno Setup), `make-resources.sh` |
| `packaging/linux/` | `build-linux.sh`, `install.sh` / `uninstall.sh` (nach `~/.local`), `.desktop`-Datei, LIESMICH |
| `packaging/macos/` | `build-macos.sh` (universelle `Klassenwerk.app`, ad-hoc signiert), `Info.plist`, LIESMICH |
| `packaging/third_party_licenses.py` | erzeugt `THIRD_PARTY_LICENSES.txt` aus `Cargo.lock` |
| `.github/workflows/build.yml` | Prüfung (fmt, Clippy, Tests), Pakete für alle drei Systeme, bei Tag `v*` ein Release |
| `docs/` | diese Seite, `REGELN.md`, `screenshots/` (README-Bilder) |
| nicht versioniert | `app/target/`, `dist/` (fertige Pakete), `_archiv/`, `app/.cargo/` (lokale Build-Einstellungen) |

### Quellcode (`app/src`)

| Datei | Aufgabe |
|---|---|
| `main.rs` | Start, Fenster, Kommandozeilen-Modi (`--mcp`, Test- und Screenshot-Modi) |
| `app.rs` | Hauptzustand `App`, Reiter, Aktionen (`enum Act`), Speichern samt Übernahme von KI-Einträgen, Import/Export, Word-Menü |
| `app/pool.rs` | Aufgabenpool, Detailansicht, Editor mit Werkzeugleiste, Zwischenablage, Formeldialog |
| `app/sheets.rs` | Klausuren und Übungen: Archiv, Zusammenstellen, Parallelklausur, Vorschau, Ergebnisse (auch je Teilaufgabe, CSV) |
| `app/classes.rs` | Reiter „Klassen & Notenbuch“, Halbjahre, Diagramm der Notenentwicklung |
| `app/misc.rs` | Übersicht, Dialoge, Einstellungen, Zurückholen aus Sicherungen, Austauschordner |
| `model.rs` | Datenmodell (`Task` mit `Part`, `Sheet`, `ResultRow` mit `scores`, `GradeScheme`, `Template`, `Settings`, `Database`), Speichern, Sicherungen, XML, `make_parallel`, `merge_external`, `score_columns`, `$…$` im Editortext |
| `classes.rs` | `Course` (mit `Assessment` und Zeugnisnoten), `Student`, Namenslisten, Notenbuch, CSV-Import und -Export, Druck |
| `markup.rs` | Textauszeichnung → Bildschirm und HTML |
| `table.rs` | Tabellen: verbundene Zellen, Rahmen je Zelle, Befehle des Tabellenmenüs |
| `rvf.rs`, `delphi.rs` | RichView-Format von KlasseDozent ↔ Textauszeichnung; Delphi-Property-Streams |
| `formula.rs` | Formeln des alten Formeleditors (WMF) → Zeichenbefehle → Bildschirm / SVG |
| `math.rs` | Formeleditor: Parser, Satz (auch Matrizen, `cases`, ausgerichtete Zeilen), Zeichenbefehle; OMML für Word |
| `wmf.rs` | Zeichenbefehle → WMF (für RTF) |
| `export.rs` | Dokumente zusammensetzen (Vorlagen, Platzhalter, Notenschlüssel, Notenspiegel mit Diagramm, Erwartungshorizont) → HTML, RTF |
| `docx.rs` | Word-Export mit eigenem ZIP-Schreiber |
| `moodle.rs` | Export als Moodle-XML |
| `wordimport.rs` | Import aus .docx (eigener Zip-Leser, Word-Formeln → Formelquelltext) und .rtf, Aufteilen an Markierungen |
| `mcp.rs` | MCP-Server |
| `vary.rs` | Aufgabenvarianten: Variablen, Zufallswerte je Mappe (`Sheet.seed`), Ausdrücke `{=…}` |
| `spell.rs` | Rechtschreibprüfung: Hunspell-Wörterbücher lesen, prüfen, Vorschläge; Suche nach installierten Wörterbüchern |
| `thesaurus.rs` | Thesaurus im MyThes-Format lesen, Synonyme (auch zu gebeugten Formen) |
| `update.rs` | Updater: neueste Version bei GitHub abfragen, passende Datei wählen, herunterladen (curl, unter Windows auch PowerShell), Installer starten |
| `fonts.rs` | Systemschriften je Plattform |
| `tests.rs` | Unit-Tests |

### Daten und Formate

Alles liegt in einer JSON-Datei `datenbank.json`:
- Windows: `%APPDATA%\Klassenwerk\`
- macOS: `~/Library/Application Support/Klassenwerk/`
- Linux: `$XDG_DATA_HOME/Klassenwerk/` bzw. `~/.local/share/Klassenwerk/`

Neue Felder bekommen immer einen Default, damit ältere Dateien weiter laden (Test `old_database_files_still_load`). Daten von Vorab-Versionen aus `%APPDATA%\KlDozent` werden beim ersten Start einmalig übernommen (`Database::migrate_old_data`).

**Textauszeichnung:** `**fett**`, `*kursiv*`, `++unterstrichen++`, `x^{2}`, `x_{1}`, `[ ]`/`[x]`, `•`, `#`, `---`, `| a | b |` (ohne Rahmen) / `+| … |` (mit Rahmen), in Tabellen `<` (mit der Zelle links verbunden), `^` (mit der Zelle darüber verbunden) und `!` vor dem Inhalt (Rahmen dieser Zelle umgeschaltet), `{blau:Text}`, `[[Linien:5]]`, `[Text](url)`.

**Eingebettete Objekte:** `{{n}}` im Text verweist auf ein `DocObject`, also `Image`, `Formula` (alter Formeleditor, WMF; auch das Notenspiegel-Diagramm) oder `Math` (neuer Formeleditor, Quelltext). Im Aufgabeneditor stehen `Math`-Formeln als `$Quelltext$` im Text, ein Dollarzeichen als `\$` (`Doc::to_dollar_text` / `from_dollar_text`); gespeichert wird weiter `{{n}}`.

**Teilaufgaben und Punkte je Aufgabe:** `Task.parts` (Bezeichnung, Punkte, Erwartung); `Task.points` ist dann ihre Summe (`sync_points`). Ergebnisse je Aufgabe stehen in `ResultRow.scores` unter dem Schlüssel von `Database::score_columns` (Aufgaben-ID, dazu `:Teil` und bei doppelt verwendeten Aufgaben `#n`), `points` ist dann ihre Summe.

**Zufallszahlen:** `Task.vars` (Name, von, bis, Schritt). `vary::values(task, seed)` wählt die Werte (gleicher Seed, gleiche Werte), `Task::varied(seed)` setzt sie in Text, Lösung, Erwartung und `Math`-Formeln ein. `export::compose` nutzt `Sheet.seed`; `make_parallel` gibt der neuen Gruppe einen anderen Seed. Pool und Einzeldruck zeigen Beispielwerte (Seed 0). Im XML-Export bleibt `{=…}` als Text stehen.

**Beispielaufgaben:** `model::task_from_spec` liest eine Aufgabe aus derselben Beschreibung, die das MCP-Werkzeug `aufgabe_anlegen` annimmt; `sample_tasks` lädt so `app/samples/beispielaufgaben.json`. Der Kopf des KlasseDozent-XML-Formats (Feldliste) steht als `XML_HEAD` in `model.rs`.

**Updates:** `App::check_update` fragt in einem Hintergrund-Thread `https://api.github.com/repos/buchhems/klassenwerk/releases/latest` ab (Einstellung `update_check`, Tag der letzten Prüfung in `update_last`, übersprungene Version in `update_skip`). Testversionen (prerelease) und Entwürfe werden nicht angeboten. Die Beschreibung eines Releases schreibt der Bauauftrag aus dem Abschnitt der Version in `CHANGELOG.md` (Überschrift `## X.Y.Z – Datum`); leere Felder (`null`) verträgt der Updater. Die Datei wird nach dem Namen gewählt, den der Bauauftrag vergibt (`…-windows-setup.exe`, `…-windows-portable.zip`, `…-macos.zip`, `…-linux-<arch>.tar.gz`), und nach dem Download auf ihre Größe geprüft. Der Installer läuft mit `/SILENT /CLOSEAPPLICATIONS` (bei einer Installation für alle Benutzer mit `/ALLUSERS`) und startet Klassenwerk danach neu.

**Tabellen:** `markup::parse` liefert je Zelle eine `table::CellInfo` (Spalten-/Zeilenspanne, 0 = verdeckt, Rahmen). Verbundene Bereiche werden zum umschließenden Rechteck ergänzt. HTML nutzt `colspan`/`rowspan`, RTF `\clvmgf`/`\clvmrg` und breitere Zellen, Word `gridSpan`/`vMerge` und `tcBorders`. Die Befehle des Menüs (`table::apply`) schreiben den Tabellentext neu und setzen den Cursor in dieselbe Zelle.

**Schrift je Aufgabe:** `Task.font` und `Task.font_size` (leer/0 = Standard). `export::Section` trägt sie; HTML setzt sie als Stil, RTF als zusätzliche Schrift in der Schrifttabelle, Word als `rFonts`/`sz` an jedem Textlauf. Auf dem Bildschirm wirkt nur die Größe.

**Sicherungen:** `Database::backup_files` listet `datenbank.1.json` … (neueste zuerst), `restore_part` übernimmt gewählte Einträge mit ihren IDs (vorhandene werden ersetzt, fehlende ergänzt, fehlende Aufgaben einer gewählten Klausur kommen mit) und hält `next_id` über den IDs der Sicherung.

**Moodle und Austauschordner:** `moodle::export` schreibt Kategorien `Klassenwerk/Fach/Thema` und je Aufgabe eine Frage vom Typ `essay` (`defaultgrade` = Punkte, Musterlösung als `generalfeedback`, Teilaufgaben-Tabelle und Musterlösung als `graderinfo`). Bilder und alte Formelbilder (als SVG) liegen als `<file>` mit `@@PLUGINFILE@@` in der Datei, neue Formeln stehen als `\( … \)` (mehrzeilige als `aligned`). Der Austauschordner (`Settings.exchange_dir`) enthält gewöhnliche `.kwerk`-Pakete, die zusätzlich Absender und Zeitpunkt tragen (`Bundle.author`, `created`); Doppelte erkennt der Import an `Task::dup_key`.

**Rechtschreibung:** Das Wörterbuch wird beim Start im Hintergrund geladen (`App::start_spell`). Gelesen werden Präfixe und Suffixe mit Bedingungen (auch ein zweites Suffix über Fortsetzungsklassen), Flag-Typen, AF-Aliase, FORBIDDENWORD, NEEDAFFIX, ONLYINCOMPOUND und Komposita über COMPOUNDFLAG/BEGIN/MIDDLE/END; zusätzlich gilt ein langes Wort aus bekannten Wörtern (mit Fugen-s) als richtig. Nicht gelesen werden COMPOUNDRULE, REP-Tabellen und Morphologie. Im Zweifel gilt ein Wort als richtig.

**Notenbuch:** Klausuren und `Course.assessments` (mündlich/sonstig, Noten als Text wie „2-“ oder „11“) werden getrennt gewichtet gemittelt und mit `Course.written_percent` (Standard 50) zusammengeführt. Der Zeugnisvorschlag ist der gerundete Gesamtdurchschnitt (x,5 aufwärts), begrenzt auf die Noten des Schlüssels der ersten Klausur; eine Eingabe in `Course.report` ersetzt ihn. Halbjahre: `classes::gradebook_term` (0 = Schuljahr, 1, 2); Klausuren zählen nach ihrem Datum (August–Januar = 1. Halbjahr), sonstige Leistungen nach `Assessment.term` oder Datum, Zeugnisnoten werden je Halbjahr gespeichert (`StudentGrade.term`).

## 4. Bauen, Testen, Veröffentlichen

```bash
cd app
cargo build --release
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test                                      # 54 Unit-Tests
./target/release/Klassenwerk --export-test DIR  # Druck, Word, Notenspiegel, Erwartungshorizont, Notenbuch, Formeln als Dateien; XML-Rundlauf
./target/release/Klassenwerk --dump             # Beispielaufgaben als Text
./target/release/Klassenwerk --mcp [--mcp-db PFAD]
./target/release/Klassenwerk --screenshot X.png [--tab pool|sheets|results|classes|stats] [--task ID]
    [--demo] [--dark] [--open settings:N|edit:ID|formula:ID|chart:HALBJAHR|update]   # edit:0 = neueste Aufgabe
```

**Pakete:**
```bash
sh packaging/linux/build-linux.sh                     # Linux → dist/Klassenwerk-<v>-linux-<arch>.tar.gz
sh packaging/macos/build-macos.sh                     # Mac   → dist/Klassenwerk-<v>-macos.zip
powershell -File packaging\windows\build-windows.ps1  # Windows → Zip + Installer (Inno Setup 6)
```
Einfacher geht es auf GitHub: Actions → „Klassenwerk bauen“ baut alle drei Systeme. Ein Tag `vX.Y.Z` erzeugt ein Release mit allen Paketen.

**Neue Version:**
1. Version in `app/Cargo.toml` erhöhen.
2. `sh packaging/windows/make-resources.sh` ausführen, sonst schlägt der Test `windows_resources_match_version` fehl.
3. `CHANGELOG.md` ergänzen.
4. Tag `vX.Y.Z` setzen und hochladen.

**Release-Profil:** `opt-level = "s"`, LTO, `codegen-units = 1`, `strip`, `panic = "abort"`. Die Programmdatei wird dadurch etwa 45 % kleiner (Linux 18,9 → 10,4 MB), ohne spürbaren Geschwindigkeitsverlust.

**Technische Hinweise:**
- Der GitHub-Bauauftrag nutzt unter Windows MSVC. Für die GNU-Toolchain bindet `build.rs` das Symbol als COFF-Objekt ein und kopiert `shlwapi.dll`. Ein eventuell nötiger `dlltool`-Pfad gehört in die lokale, nicht versionierte `app/.cargo/config.toml`. Scheitert das Einbinden des Symbols, hilft `KLASSENWERK_NO_ICON=1`.
- Clippy ist im Bauauftrag auf eine feste Rust-Version gepinnt, damit neue Clippy-Regeln den Build nicht unerwartet brechen. Beim Anheben der Version vorher lokal prüfen.
- Ohne Windows testen: Unter Linux startet das Programm mit `xvfb-run`. Die Screenshot-Hilfen (`--demo`, `--open`, `--tab results`) zeigen Notenbuch mit mündlichen Noten, Ergebnisse je Teilaufgabe, Formeldialog und Einstellungen ohne Klicken. Umgebungsvariablen für Screenshots: `KLASSENWERK_FORMULA` (Formel im Formeldialog), `KLASSENWERK_SPELLDICT` (Wörterbuch), `KLASSENWERK_HEIGHT` (Fensterhöhe).

## 5. Stand der Prüfung und bekannte Einschränkungen

**Noch nicht von Hand geprüft:**
- Windows-Oberfläche der neueren Funktionen (Formeleditor, Gruppe A/B, Notenbuch, KI-Einstellungen). Unter Linux sind sie per Screenshot geprüft, Logik und Exporte durch Tests abgedeckt.
- Word-Formeln (OMML) in echtem Word, besonders Matrizen und ausgerichtete Zeilen; geprüft ist nur die XML-Struktur. Das Notenspiegel-Diagramm ist in LibreOffice geprüft.
- CSV-Import mit Dateien aus echtem Excel (getestet mit Semikolon, Tabulator, Anführungszeichen, UTF-8 und Windows-1252).
- Updater unter Windows, macOS und Linux mit einem echten Release (geprüft sind Versionsvergleich, Dateiwahl, Antwort der GitHub-API und der Download mit curl).
- Rechtschreibprüfung mit dem echten deutschen Wörterbuch (de_DE_frami von LibreOffice): Hier stand nur das englische Wörterbuch und ein kleines deutsches Testwörterbuch zur Verfügung. Besonders Komposita können anders als in LibreOffice bewertet werden.
- MCP-Server mit dem echten Claude Desktop; geprüft ist eine vollständige Protokollsitzung.
- macOS-Build auf einem echten Mac.
- Farbige Texte im Druck und in Word.

**Einschränkungen:**
- Installer, `.exe` und macOS-App sind nicht signiert bzw. nicht notarisiert. Windows zeigt „Unbekannter Herausgeber“, unter macOS beim ersten Start Rechtsklick → Öffnen.
- RTF: Neue Formeln werden per `\dn` abgesenkt; LibreOffice ignoriert das.
- Teilaufgaben, Punkte je Aufgabe, mündliche Noten und Zeugnisnoten gibt es nur in Klassenwerk: Der XML-Export im Format von KlasseDozent lässt sie weg (Aufgabenpakete `.kwerk` behalten sie).
- Der Text der Teilaufgaben steht weiter frei in der Aufgabe; Klassenwerk prüft nicht, ob er zu den angelegten Teilaufgaben passt.
- Zufallszahlen: keine Bedingungen zwischen Variablen (z. B. „Diskriminante ≥ 0“); solche Fälle über passende Bereiche lösen.
- Der Updater braucht `curl` (bei Windows 10/11, macOS und Linux vorhanden) oder unter Windows PowerShell. Installer und Programm sind nicht signiert; Windows kann beim Update nach einer Bestätigung fragen.
- Die Rechtschreibprüfung und der Thesaurus brauchen installierte Wörterbücher (LibreOffice oder unter Linux hunspell-de-de / mythes-de); Klassenwerk bringt keine mit. Beides gibt es nur im Aufgabeneditor. Der Thesaurus findet gebeugte Formen nur über einfache Endungen („Hunden“ → „Hund“, nicht „Häuser“ → „Haus“).
- Schrift je Aufgabe: Auf dem Bildschirm ändert sich nur die Größe; die Schriftart muss beim Drucken auf dem Rechner installiert sein. Der XML-Export im Format von KlasseDozent lässt sie weg.
- Verbundene Tabellenzellen: Was nicht rechteckig verbunden ist, wird zum umschließenden Rechteck ergänzt (Text darin entfällt). Im Bildschirm-Editor steht die Tabelle weiter als Text.
- Word-Import: alte .doc-Dateien vorher als .docx speichern; MathType- und andere OLE-Formeln kommen als Bild (WMF) oder fehlen (EMF); Formeln in RTF-Dateien nur, wenn ein WMF-Ersatzbild darin steht.
- Moodle: Exportiert werden Freitextfragen (bewertet von Hand); Formeln brauchen in Moodle den MathJax-Filter (Standard). Zufallszahlen erscheinen mit den Beispielwerten.
- Austauschordner: Es gibt keine Rechte oder Sperren; wer Zugriff auf den Ordner hat, kann Pakete ablegen und löschen.
- Formeln: Matrizen ohne Spaltenformat (`array{cc}` wird gelesen, aber nicht ausgewertet); `$…$` gilt nur im Aufgabeneditor, nicht in Zusatztexten und Druckvorlagen.
- Speichern App und MCP-Server in derselben Millisekunde, kann ein KI-Eintrag erst beim nächsten Abgleich erscheinen.

## 6. Weiterentwicklung

**Nächste Schritte (höchster Nutzen):**
1. Zufallszahlen mit Bedingungen und abgeleiteten Variablen
2. Rechtschreibprüfung und Thesaurus auch in Zusatztexten und Druckvorlagen
3. Moodle: weitere Fragetypen (Multiple Choice aus Ankreuzkästchen, numerische Fragen mit Zufallszahlen)

**Später:**
- MCP: Entwürfe ändern, anonymisierter Notenspiegel, Prompt-Vorlagen
- `$…$` auch in Zusatztexten und Druckvorlagen
- Technik: `app.rs` aufteilen; Bilder aus der JSON-Datei in eigene Dateien oder SQLite auslagern; Code-Signatur für Windows und macOS

**Funktionen von KlasseDozent, die bewusst noch fehlen:** Datenbank-Login und Mehrbenutzerbetrieb (eine gemeinsame Datenbank für mehrere Lehrkräfte). Aufgaben teilen geht über den Austauschordner.

## 7. Regeln für Änderungen

- Nach jeder Änderung: `cargo test` grün, `--export-test` mit XML-Rundlauf 25 → 25, betroffene Ansichten ansehen (unter Linux per `--screenshot`).
- Keine fremden Inhalte ins Repository: Beispielaufgaben, Bilder und Testdaten müssen selbst erstellt oder frei lizenziert sein. Echte KlasseDozent-Exporte zum Testen gehören nach `_archiv/musteraufgaben-original/` (nicht versioniert); der Test `real_klassedozent_exports_if_present` prüft sie, wenn sie da sind.
- Regeln für Noten und Druck nur zusammen mit `docs/REGELN.md` und den Tests ändern.
- Oberfläche auf Deutsch, mit den Begriffen von KlasseDozent (Schlagwort 1/2, Notenschlüssel, unterstreichen …).
- Neue Datenfelder immer mit Default.
- Schülerdaten dürfen nie über den MCP-Server erreichbar werden (Test `mcp_never_shows_student_data` bei neuen Datenfeldern erweitern).
