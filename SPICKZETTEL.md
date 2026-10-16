# Rust-Spickzettel (mit Python-Vergleich)

Alles, was im Code neu war, kurz erklärt. Die erste Fassung vom 2026-10-15 ist verloren gegangen (sie war nie in git) –
diese hier fängt am 2026-10-16 neu an.

## Module und Sichtbarkeit

| Rust | Was es heißt | Python |
|---|---|---|
| `mod zombies;` | Bindet die Datei `zombies.rs` (im selben Ordner) als Modul ein. Ohne diese Zeile gehört die Datei nicht zum Programm. | Ein Modul existiert, sobald die Datei da ist – `import zombies` |
| Ordner `sim\` mit `mod.rs` | Ein Modul aus mehreren Dateien: `mod.rs` ist das Modul `sim` selbst, die anderen Dateien darin sind seine Kinder (`sim::zombies`). | Paket-Ordner mit `__init__.py` |
| privat (kein `pub`) | Sichtbar im eigenen Modul **und in allen Kind-Modulen** – deshalb sehen `zombies.rs` & Co. die privaten Felder von `World` aus `mod.rs`. | Es gibt kein echtes privat, nur die Abmachung `_name` |
| `pub(super)` | Sichtbar im Eltern-Modul – und damit auch bei allen Geschwistern. Für das, was die Dateien von `sim\` untereinander brauchen, was der Rest des Spiels aber nicht sehen soll. | – |
| `pub` | Für alle sichtbar. | normaler Name ohne `_` |
| `pub use units::Unit;` | Re-Export: holt `Unit` aus dem Kind-Modul und bietet es unter `sim::Unit` an. So bleibt `use crate::sim::Unit` überall gleich, egal in welcher Datei `Unit` wirklich steht. | `from .units import Unit` in `__init__.py` |
| `use super::World;` | `super` = das Eltern-Modul (wie `..` bei Ordnern). | `from .. import World` |
| `impl World { … }` in mehreren Dateien | Methoden einer Struct dürfen auf viele `impl`-Blöcke in verschiedenen Dateien verteilt sein (solange es im selben Crate ist). | Eine Klasse steht in einer Datei; verteilen ginge nur mit Mixins |

Rust-Buch: Kapitel 7 („Wachsende Projekte mit Paketen, Crates und Modulen verwalten“), besonders 7.3–7.5.

## Bits (Bitmasken)

| Rust | Was es heißt | Python |
|---|---|---|
| `u8` | Ganze Zahl ohne Vorzeichen, 8 Bit (0–255) – ein Byte. Für 5 Ja/Nein-Werte pro Kachel reicht eins. | Python hat nur `int` (beliebig groß) |
| `1 << 3` | Eine 1, drei Stellen nach links geschoben = `0b0000_1000` = 8. So bekommt jede Eigenschaft ihr eigenes Bit. | gleich: `1 << 3` |
| `a \| b` | Bitweises Oder: alle Bits, die in `a` **oder** `b` gesetzt sind. `SOLID \| BUILT` = „eins von beiden“. | gleich: `a \| b` |
| `a & b` | Bitweises Und: nur die Bits, die in beiden gesetzt sind. `(flags & DOOR) != 0` = „ist das Tür-Bit gesetzt?“ | gleich: `a & b` |
| `!a` (bei Zahlen) | Bitweises Nicht: jedes Bit umgedreht. `flags &= !(BUILT \| DOOR)` löscht genau diese zwei Bits. (Bei `bool` ist `!` das normale „nicht“.) | `~a` |
| `flags \|= BUILT` | Bit setzen (kurz für `flags = flags \| BUILT`). | gleich |
| `(flags & DOOR) != 0` | In Rust bindet `&` stärker als `!=`, die Klammern sind also nicht nötig (anders als in C) – sie stehen trotzdem da, weil man es so schneller liest. | In Python bindet `&` auch stärker als `!=` |
| `let x = if a { 1 } else { 0 };` | `if` ist ein Ausdruck und liefert einen Wert. | `x = 1 if a else 0` |

Rust-Buch: Anhang B („Operatoren und Symbole“).

## Konstanten, Zahlen, Tests

| Rust | Was es heißt | Python |
|---|---|---|
| `const NAME: u64 = 5;` | Eine Konstante: Typ muss dabei stehen, der Wert steht schon beim Kompilieren fest, Name in GROSSBUCHSTABEN. | `NAME = 5` (nur Abmachung, änderbar) |
| `0x2e47_304e_ea00_38a9` | Zahl in Hexadezimal (`0x`), `_` nur zum Lesen – wird ignoriert. | gleich: `0x2e47_304e` |
| `x.to_bits()` | Die rohen Bits eines `f32` als `u32` – zwei Floats sind genau dann bitgleich, wenn das gleich ist. | `struct.unpack('I', struct.pack('f', x))` |
| `libm::cosf(a)` | Eine Funktion aus einer Bibliothek (Crate) direkt mit Pfad aufrufen, ohne `use`. `f` = die `f32`-Version. | `math.cos(a)` |
| `#[test]` | Markiert eine Funktion als Test; `cargo test` führt alle aus. | `def test_…()` mit pytest |
| `assert_eq!(a, b, "text {x:#018x}")` | Bricht ab, wenn `a != b`, und zeigt beide Werte plus den Text. `{x:#018x}` = `x` als Hex mit `0x`, 18 Zeichen breit, vorne Nullen. | `assert a == b, f"text {x:#018x}"` |

Rust-Buch: Kapitel 3.1 (Konstanten), 3.2 (Zahlen), 11 („Automatisierte Tests schreiben“).
