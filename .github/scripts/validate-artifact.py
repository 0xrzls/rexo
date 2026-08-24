#!/usr/bin/env python3
"""
Validator artifact Rialo.

Kenapa ini ada: assertion lamaku hanya menghitung file `*.wasm`. File wasm
KOSONG — nol fungsi, hanya mengekspor `memory` — tetap lolos hitungan itu.
Persis itu yang terjadi di run 9532535523.

Validator ini membuka file-nya dan memeriksa isinya.

Pakai:  python3 validate-artifact.py build-output/
Keluar 0 kalau ada artifact yang benar-benar dapat di-deploy, 1 kalau tidak.
"""

import sys
import pathlib

CODE_SECTION = 10
FUNC_SECTION = 3
EXPORT_SECTION = 7
KIND = ["func", "table", "memory", "global"]


def uleb(buf, pos):
    result = shift = 0
    while True:
        byte = buf[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return result, pos


def inspect_wasm(path):
    """Kembalikan (ok, alasan, ringkasan)."""
    data = path.read_bytes()

    if data[:4] != b"\x00asm":
        return False, "bukan file wasm (magic salah)", {}

    version = data[4:8]
    kind = {
        b"\x01\x00\x00\x00": "core module",
        b"\x0d\x00\x01\x00": "component",
    }.get(version, f"tidak dikenal ({version.hex()})")

    pos = 8
    sections = {}
    func_count = 0
    exports = []

    while pos < len(data):
        sid = data[pos]
        pos += 1
        size, pos = uleb(data, pos)
        body = data[pos : pos + size]
        sections[sid] = sections.get(sid, 0) + size

        if sid == FUNC_SECTION:
            func_count, _ = uleb(body, 0)
        elif sid == EXPORT_SECTION:
            count, p = uleb(body, 0)
            for _ in range(count):
                nlen, p = uleb(body, p)
                name = body[p : p + nlen].decode("utf8", "replace")
                p += nlen
                k = KIND[body[p]] if body[p] < len(KIND) else "?"
                p += 1
                _, p = uleb(body, p)
                exports.append((name, k))
        pos += size

    debug_bytes = 0
    # Semua custom section (id 0) yang besar biasanya DWARF.
    if 0 in sections:
        debug_bytes = sections[0]

    summary = {
        "kind": kind,
        "size": len(data),
        "functions": func_count,
        "has_code": CODE_SECTION in sections,
        "exports": exports,
        "func_exports": [n for n, k in exports if k == "func"],
        "debug_bytes": debug_bytes,
    }

    if CODE_SECTION not in sections or func_count == 0:
        return False, "NOL fungsi — modul kosong, tidak ada code section", summary
    if not summary["func_exports"]:
        return False, "tidak mengekspor satu pun fungsi", summary

    return True, "ok", summary


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "build-output")
    if not root.is_dir():
        print(f"::error::direktori tidak ada: {root}")
        return 1

    blobs = [p for p in root.rglob("*") if p.suffix in (".polkavm", ".blob")]
    wasms = [p for p in root.rglob("*.wasm")]

    print(f"Memeriksa {root}/")
    print(f"  blob PolkaVM : {len(blobs)}")
    print(f"  modul wasm   : {len(wasms)}")
    print()

    failures = []

    for p in blobs:
        size = p.stat().st_size
        status = "ok" if size > 0 else "KOSONG"
        print(f"  [{status}] {p.name}  {size:,} bytes")
        if size == 0:
            failures.append(f"{p.name}: file nol byte")

    for p in wasms:
        ok, reason, s = inspect_wasm(p)
        mark = "ok" if ok else "GAGAL"
        print(f"  [{mark}] {p.name}  {s.get('size', 0):,} bytes  ({s.get('kind', '?')})")
        if s:
            print(f"          fungsi   : {s['functions']}")
            print(f"          code sec : {s['has_code']}")
            print(f"          ekspor   : {[n for n, _ in s['exports']] or 'tidak ada'}")
            if s["debug_bytes"] > s["size"] * 0.5:
                pct = 100 * s["debug_bytes"] / s["size"]
                print(f"          catatan  : {pct:.0f}% isinya debug info")
        if not ok:
            print(f"          alasan   : {reason}")
            failures.append(f"{p.name}: {reason}")
        print()

    # -----------------------------------------------------------------
    # Gerbang utama. Blob PolkaVM adalah artifact Venus yang sebenarnya
    # dapat di-deploy. Modul wasm hanyalah komponen REX pendukung.
    # Tanpa blob, tidak ada yang bisa dikirim ke chain.
    # -----------------------------------------------------------------
    if not blobs:
        print("::error::Tidak ada blob PolkaVM (.polkavm/.blob). "
              "Program Venus tidak terkompilasi — file .wasm saja tidak dapat di-deploy.")
        return 1

    if failures:
        for f in failures:
            print(f"::error::{f}")
        return 1

    print("Semua artifact valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
