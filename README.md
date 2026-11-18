# Klassenwerk

Aufgabendatenbank, Klausurplaner und Notenbuch für Lehrkräfte – ein moderner Neubau von KlasseDozent 4.8, geschrieben in Rust mit egui. Läuft unter Windows, Linux und macOS.

![Klassenwerk – Aufgabenpool mit Aufgabe und Musterlösung](docs/screenshots/aufgabenpool.png)

## Warum Klassenwerk?

KlasseDozent ist ein bewährtes Werkzeug für Aufgaben, Klausuren und Noten. Klassenwerk ist entstanden, damit Lehrkräfte ihre Aufgabensammlung und ihre gewohnte Arbeitsweise weiter nutzen können – und zwar aus drei Gründen:

1. **Mac und Linux:** KlasseDozent gab es nur für Windows. Klassenwerk läuft unter Windows, macOS und Linux.
2. **KI-Assistent über MCP:** Ein Assistent wie Claude kann über das Model Context Protocol Aufgaben suchen und neue Aufgaben und Klausurentwürfe anlegen, ohne Zugriff auf Schülerdaten. Dazu kommen weitere neue Funktionen, unter anderem Teilaufgaben mit Erwartungshorizont, Zufallszahlen für Gruppe A/B, ein Notenbuch mit Zeugnisnote, Word- und Moodle-Export und der Import aus Word.
3. **Weiterentwicklung:** Es scheint, als gäbe es für KlasseDozent keine Updates mehr. Klassenwerk wird weiter gepflegt und aktualisiert sich auf Wunsch selbst.

Aufgaben aus KlasseDozent lassen sich über dessen XML-Export übernehmen; Word-Dokumente mit den Markierungen von KDWord („Aufgabe=1“, „Lösung=1“ …) liest Klassenwerk direkt.

**Funktionen:** Aufgaben mit Musterlösungen, Teilaufgaben und Zufallszahlen (jede Gruppe bekommt andere Zahlen) sammeln, Klausuren und Übungen zusammenstellen (auch als Parallelklausur Gruppe A/B), mit Formeleditor (auch Matrizen und mehrzeilige Rechnungen), Notenschlüssel, Notenspiegel-Diagramm und Erwartungshorizont drucken oder als Word-Datei speichern, Aufgaben aus Word-Dokumenten übernehmen, Punkte je Teilaufgabe erfassen oder aus Excel einlesen, im Notenbuch Klausuren und mündliche Noten je Halbjahr zur Zeugnisnote verrechnen, mit Rechtschreibprüfung im Editor. Neue Versionen findet und installiert Klassenwerk auf Wunsch selbst. Auf Wunsch arbeitet ein KI-Assistent über MCP mit, ohne Zugriff auf Schülerdaten. Einzelheiten stehen in [CHANGELOG.md](CHANGELOG.md).

## Bilder

| Klausur zusammenstellen, Gruppe A/B | Ergebnisse je Teilaufgabe |
|---|---|
| ![Klausur bearbeiten](docs/screenshots/klausur.png) | ![Ergebnisse je Teilaufgabe](docs/screenshots/ergebnisse.png) |
| **Notenbuch mit mündlichen Noten und Zeugnisnote** | **Formeleditor mit Matrix und Fallunterscheidung** |
| ![Notenbuch](docs/screenshots/notenbuch.png) | ![Formeleditor](docs/screenshots/formeleditor.png) |
| **Dunkles Design** | |
| ![Dunkles Design](docs/screenshots/dunkel.png) | |

Die Namen in den Bildern sind erfunden. Die Beispielaufgaben, die Klassenwerk beim ersten Start mitbringt, sind eigens für das Projekt geschrieben und wie das Programm gemeinfrei.

## Bauen

```bash
cd app
cargo build --release     # Programm: app/target/release/Klassenwerk(.exe)
cargo test
```

Pakete: `packaging/linux/build-linux.sh` (Linux), `packaging/macos/build-macos.sh` (auf einem Mac) oder der GitHub-Bauauftrag `.github/workflows/build.yml`, der alle drei Systeme baut.

## KI-Assistent

`Klassenwerk --mcp` startet den MCP-Server. Einrichten lässt er sich in Klassenwerk unter Einstellungen → „KI-Assistent“ (Claude Desktop, Claude Code).

## Lizenz

Klassenwerk ist gemeinfrei ([CC0 1.0](LICENSE)): Jeder darf es ohne Bedingungen nutzen, verändern und weitergeben, auch kommerziell. Wer es weiterverwendet, wird um einen Hinweis auf das Projekt gebeten; das ist aber keine Pflicht.

Klassenwerk wurde mit Claude (Anthropic) entwickelt, als KI-gestützter Neubau nach dem Vorbild von KlasseDozent. Deshalb beansprucht niemand ein Urheberrecht daran. Die Lizenzen der verwendeten Bibliotheken und Schriften stehen in [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt).

## Dokumentation

- [docs/PROJEKT_ZUSAMMENFASSUNG.md](docs/PROJEKT_ZUSAMMENFASSUNG.md): Aufbau, Bauen und Testen, offene Punkte, Ideen
- [docs/REGELN.md](docs/REGELN.md): Regeln für Notenschlüssel und Druck, Datenfelder des XML-Formats
