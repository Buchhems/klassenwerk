#!/usr/bin/env python3
"""Erzeugt THIRD_PARTY_LICENSES.txt: alle Rust-Pakete aus app/Cargo.lock mit Lizenz,
Urheberangaben und Lizenztexten (aus dem lokalen Cargo-Cache).
Aufruf (nach `cargo fetch` im Ordner app):  python3 packaging/third_party_licenses.py"""
import glob, io, os, re, sys, tarfile

try:  # Python 3.11+
    import tomllib
except ImportError:  # older Python (e.g. Ubuntu 22.04): small reader for the few keys needed here
    tomllib = None

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARGO = os.environ.get("CARGO_HOME", os.path.expanduser("~/.cargo"))


def _str(v):
    v = v.strip()
    if v.startswith('"""'):
        return v[3:].split('"""')[0]
    if v.startswith('"'):
        return re.match(r'"((?:[^"\\]|\\.)*)"', v).group(1).replace('\\"', '"')
    if v.startswith("'"):
        return v[1:].split("'")[0]
    return v


def lock_packages(text):
    if tomllib:
        return [(p["name"], p["version"], p.get("source", "")) for p in tomllib.loads(text)["package"]]
    out = []
    for block in text.split("[[package]]")[1:]:
        f = dict(re.findall(r'^(name|version|source) = "([^"]*)"', block, re.M))
        out.append((f.get("name"), f.get("version"), f.get("source", "")))
    return out


def package_meta(text):
    """license, license-file and authors from the [package] table of a Cargo.toml"""
    if tomllib:
        return tomllib.loads(text).get("package", {})
    m = re.search(r"^\[package\][ \t]*$(.*?)(?=^\[)", text + "\n[", re.S | re.M)
    sec = m.group(1) if m else ""
    meta = {}
    for key in ("license", "license-file"):
        k = re.search(r"^" + re.escape(key) + r"\s*=\s*(.+)$", sec, re.M)
        if k:
            meta[key] = _str(k.group(1))
    a = re.search(r"^authors\s*=\s*\[(.*?)\]", sec, re.S | re.M)
    if a:
        meta["authors"] = re.findall(r'"((?:[^"\\]|\\.)*)"', a.group(1))
    return meta


lock_text = open(os.path.join(ROOT, "app", "Cargo.lock"), encoding="utf-8").read()
pkgs = sorted({(n, v) for n, v, src in lock_packages(lock_text) if src.startswith("registry+")})

def crate_file(name, ver):
    hits = glob.glob(os.path.join(CARGO, "registry", "cache", "*", f"{name}-{ver}.crate"))
    return hits[0] if hits else None

out = io.StringIO()
missing = []
texts = {}
rows = []
for name, ver in pkgs:
    f = crate_file(name, ver)
    if not f:
        missing.append(f"{name} {ver}")
        continue
    with tarfile.open(f) as t:
        members = {m.name.split("/", 1)[1]: m for m in t.getmembers() if "/" in m.name and m.isfile()}
        meta = package_meta(t.extractfile(members["Cargo.toml"]).read().decode("utf-8", "replace"))
        lic = meta.get("license") or ("siehe Lizenzdatei" if meta.get("license-file") else "unbekannt")
        authors = ", ".join(meta.get("authors", []))
        files = sorted(n for n in members if "/" not in n and re.match(r"(?i)(licen[cs]e|copying|notice|copyright)", n))
        # fonts bundled with egui carry their own licence files in sub folders
        files += sorted(n for n in members if re.search(r"(?i)/[^/]*(ofl|ufl|licen[cs]e)[^/]*\.txt$", n) and n not in files)
        body = []
        for n in files:
            txt = t.extractfile(members[n]).read().decode("utf-8", "replace").replace("\r\n", "\n").replace("\r", "\n").strip()
            body.append((n, txt))
    rows.append((name, ver, lic, authors, body))

out.write("Klassenwerk – Lizenzen der verwendeten Fremdpakete\n")
out.write("=" * 52 + "\n\n")
out.write("Klassenwerk selbst ist gemeinfrei (CC0 1.0, siehe LICENSE).\n")
out.write("Das Programm enthält die folgenden Rust-Pakete (je nach Betriebssystem nur ein Teil davon).\n")
out.write("Ihre Lizenzen und Urheberhinweise sind unten vollständig wiedergegeben.\n\n")
w = max(len(n) for n, *_ in rows)
for name, ver, lic, authors, _ in rows:
    out.write(f"  {name.ljust(w)}  {ver.ljust(10)}  {lic}\n")
if missing:
    out.write("\nOhne Lizenztext, weil beim Erzeugen nicht im lokalen Paket-Cache (alle MIT OR Apache-2.0;\n"
              "die Standardtexte beider Lizenzen stehen unten bei anderen Paketen): " + ", ".join(missing) + "\n")
    out.write("Vollständig wird die Liste, wenn vorher `cargo fetch` (mit Internet) im Ordner app läuft.\n")
out.write("\n")
for name, ver, lic, authors, body in rows:
    out.write("\n" + "-" * 78 + f"\n{name} {ver}  –  {lic}\n")
    if authors:
        out.write(f"Autoren: {authors}\n")
    for fname, txt in body:
        key = re.sub(r"\s+", " ", txt)
        if key in texts:
            out.write(f"\n[{fname}: gleicher Text wie bei {texts[key]}]\n")
        else:
            texts[key] = f"{name} {ver} ({fname})"
            out.write(f"\n[{fname}]\n{txt}\n")
    if not body:
        out.write("\n(keine Lizenzdatei im Paket; Lizenz laut Paketangabe: " + lic + ")\n")

dst = os.path.join(ROOT, "THIRD_PARTY_LICENSES.txt")
open(dst, "w", encoding="utf-8", newline="\n").write(out.getvalue())
print(f"{dst}: {len(rows)} Pakete, {len(missing)} nicht im Cache, {os.path.getsize(dst)//1024} KB")
