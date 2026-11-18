# Regeln von Klassenwerk

Wie Klassenwerk Noten berechnet und druckt. Die Notenstufen der mitgelieferten Schlüssel folgen veröffentlichten Tabellen; Berechnung, Druckaufbau und Standardwerte sind eigene Festlegungen von Klassenwerk. Wer etwas daran ändert, passt bitte auch diese Seite und die Tests in `app/src/tests.rs` an.

## Notenschlüssel

Ein Schlüssel gibt für jede Note an, ab wie viel Prozent der Gesamtpunkte sie gilt.

| Schlüssel | Note ab … % | Quelle |
|---|---|---|
| IHK | 1 ab 92, 2 ab 81, 3 ab 67, 4 ab 50, 5 ab 30, 6 darunter | bundeseinheitlicher 100-Punkte-Schlüssel der Industrie- und Handelskammern |
| Gymnasium | 15 ab 95, 14 ab 90, 13 ab 85, 12 ab 80, 11 ab 75, 10 ab 70, 9 ab 65, 8 ab 60, 7 ab 55, 6 ab 50, 5 ab 45, 4 ab 40, 3 ab 33, 2 ab 27, 1 ab 20, 0 darunter | Zuordnung von Rohpunkten zu Notenpunkten im Zentralabitur NRW (Schulministerium NRW, Anlage 4 „Schema und Beispiele zur Notenbildung“) |
| Noten mit Tendenz (1+ bis 6) | 1+ ab 97 … 5- ab 20, 6 darunter | eigene Schätzung, bitte an die eigene Schule anpassen |

Länder und Schulen haben teils andere Tabellen; jeder Schlüssel lässt sich ändern oder neu anlegen.

**Punktgrenzen für eine Klausur**
- Nötige Punkte einer Note = Gesamtpunkte × Prozent ÷ 100, **aufgerundet** auf die Stufung des Schlüssels: halbe Punkte (Standard), ganze Punkte oder Zehntel. So bekommt niemand eine Note mit weniger als ihrem Prozentwert.
- Eine Note reicht bis eine Stufe unter den Beginn der nächstbesseren Note; die beste Note reicht bis zur Gesamtpunktzahl.
- Die Note für ein Ergebnis ist die beste, deren untere Grenze erreicht ist. Gedruckter Schlüssel und berechnete Note stimmen damit immer überein.
- Bei wenigen Gesamtpunkten kann eine Note keinen eigenen Bereich mehr haben (zum Beispiel beim 15-Punkte-Schlüssel mit 7 Punkten). Sie fehlt dann im Ausdruck; die Einstellungen weisen darauf hin.
- Grenzen erscheinen ohne Nachkommastelle, wenn sie ganz sind, sonst mit einer („28,5“).

Beispiel (31 Punkte, IHK, halbe Punkte): 1 = 29–31; 2 = 25,5–28,5; 3 = 21–25; 4 = 15,5–20,5; 5 = 9,5–15; 6 = 0–9.

**Ausdruck des Schlüssels:** eine Überschrift (Standard „Notenschlüssel“), dann waagerecht die Zeilen „Note“ und „Punkte“, auf Wunsch „ab %“ und eine leere Zeile „Anzahl“ zum Eintragen (Standard: an). Mehr als acht Noten werden auf mehrere Tabellen verteilt.

**Weitere Auswertung:** „Nicht bestanden ab Note“ legt fest, welche Noten als nicht bestanden zählen; der Notenspiegel zeigt Anzahl je Note, Durchschnitt und ein Diagramm. Ergebnisse mit „fehlt“ zählen nicht.

## Datenfelder im XML-Austauschformat von KlasseDozent

Für Import und Export der Aufgaben-Dateien von KlasseDozent:

| Feld | Bedeutung |
|---|---|
| Desc1 | Fach |
| Desc2 / Desc3 | Schlagwort 1 / Schlagwort 2 |
| Subject | Titel |
| Int1 | Leerzeilen: Anzahl = Betrag; negativ = Linien unterstreichen |
| Int2 | Punktzahl |
| Int3 / Int4 / Int5 | werden unverändert durchgereicht |
| Body1 / Body2 | Aufgabe / Lösung (RichView-Format) |

Textfelder sind in diesen Dateien doppelt UTF-8-kodiert (aus „ä“ wird „Ã¤“). Klassenwerk liest und schreibt das so.

## Standardwerte

| Einstellung | Standard | Begründung |
|---|---|---|
| Seitenränder oben/rechts/unten/links | 20 / 20 / 20 / 25 mm | links Platz zum Lochen und Abheften |
| Antwortlinien | 9 mm Abstand, unabhängig von der Schriftgröße | gleich viel Platz zum Schreiben in jeder Aufgabe |
| Punktgrenzen im Notenschlüssel | halbe Punkte | in Klausuren üblich |
| Punkte der Hauptaufgabe | Summe der Teilaufgaben | |
| Sicherungen der Datenbank | 5 Generationen | |
| Doppelte Aufgabe in einer Klausur | Warnung | |
| Druckschrift | Arial 12 pt | auf allen Rechnern vorhanden |
| Bearbeitungszeit einer neuen Klausur | 90 Minuten | |

Leerzeilen werden unterstrichen, wenn die Aufgabe es verlangt **oder** beim Drucken „Leerzeilen unterstreichen“ gewählt ist.
