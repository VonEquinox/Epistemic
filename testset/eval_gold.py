#!/usr/bin/env python3
"""Lightweight gold JSON validator / optional DB compare for M3."""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

GOLD = Path(__file__).resolve().parent / "gold"
VALID_TYPES = {
    "uses_method_from",
    "improves_on",
    "alternative_to",
    "uses_dataset_from",
    "compares_against",
    "reproduces",
    "fails_to_reproduce",
    "supports_claim",
    "contradicts_claim",
    "prerequisite_for",
    "cites",
    "version_of",
}

def load_all():
    files = sorted(GOLD.glob("*.json"))
    if not files:
        print("no gold files", file=sys.stderr)
        sys.exit(1)
    out = []
    for f in files:
        with f.open() as fh:
            out.append((f.name, json.load(fh)))
    return out


def validate(items):
    errors = []
    n_rel = 0
    n_ctx = 0
    n_claim = 0
    n_method = 0
    for name, g in items:
        if "arxiv_id" not in g:
            errors.append(f"{name}: missing arxiv_id")
        dna = g.get("dna") or {}
        for c in dna.get("claims") or []:
            n_claim += 1
            if not c.get("text"):
                errors.append(f"{name}: claim missing text")
        for m in dna.get("methods") or []:
            n_method += 1
            if not m.get("name"):
                errors.append(f"{name}: method missing name")
        for r in g.get("relations") or []:
            n_rel += 1
            t = r.get("type")
            if t not in VALID_TYPES:
                errors.append(f"{name}: invalid relation type {t}")
            if not r.get("target_arxiv"):
                errors.append(f"{name}: relation missing target_arxiv")
        for c in g.get("citation_contexts") or []:
            n_ctx += 1
            if c.get("gold_type") not in VALID_TYPES:
                errors.append(f"{name}: invalid cite gold_type {c.get('gold_type')}")
    print(f"gold papers: {len(items)}")
    print(f"  claims={n_claim} methods={n_method} relations={n_rel} cite_ctx={n_ctx}")
    if errors:
        print(f"errors ({len(errors)}):")
        for e in errors:
            print(" -", e)
        return False
    print("schema: OK")
    return True


def compare_db(items):
    url = os.environ.get("DATABASE_URL")
    if not url:
        print("DATABASE_URL not set; skip --db", file=sys.stderr)
        return True
    try:
        import psycopg2  # type: ignore
    except ImportError:
        print("psycopg2 not installed; skip --db", file=sys.stderr)
        return True
    conn = psycopg2.connect(url)
    cur = conn.cursor()
    # map arxiv -> work_id
    cur.execute("SELECT arxiv_id, work_id FROM versions WHERE arxiv_id IS NOT NULL")
    aw = {r[0]: r[1] for r in cur.fetchall()}
    # existing relations between works
    cur.execute(
        """
        SELECT r.type, ms.anchor_work_id, mt.anchor_work_id
        FROM relations r
        JOIN relation_members ms ON ms.relation_id = r.id AND ms.role = 'source'
        JOIN relation_members mt ON mt.relation_id = r.id AND mt.role = 'target'
        """
    )
    existing = {(t, str(s), str(g)) for t, s, g in cur.fetchall()}
    hit = 0
    total = 0
    missing = []
    for name, g in items:
        src = aw.get(g["arxiv_id"])
        if not src:
            continue
        for r in g.get("relations") or []:
            total += 1
            tgt = aw.get(r["target_arxiv"])
            if not tgt:
                missing.append(f"{name}: target {r['target_arxiv']} not in DB")
                continue
            key = (r["type"], str(src), str(tgt))
            if key in existing:
                hit += 1
            else:
                missing.append(f"{name}: {r['type']} -> {r['target_arxiv']}")
    print(f"DB relation coverage: {hit}/{total}")
    for m in missing[:20]:
        print(" -", m)
    cur.close()
    conn.close()
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", action="store_true")
    args = ap.parse_args()
    items = load_all()
    ok = validate(items)
    if args.db:
        compare_db(items)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
