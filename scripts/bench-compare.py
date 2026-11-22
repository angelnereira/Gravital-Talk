#!/usr/bin/env python3
"""Compara dos baselines de criterion y falla si hay regresion >= umbral.

Uso tipico en CI (mismo runner, hardware identico para ambas ramas):

    cargo bench -p gravital-talk --bench checksum --bench encode_decode -- --save-baseline base
    git checkout <head>
    cargo bench -p gravital-talk --bench checksum --bench encode_decode -- --save-baseline head
    python3 scripts/bench-compare.py --base base --head head --threshold 10

Criterion guarda `median.point_estimate` en nanosegundos en
`target/criterion/<grupo>/<bench>/<label>/estimates.json` (o sin grupo).
"""

import argparse
import json
import os
import sys


def load_estimates(root, label):
    """Devuelve {clave: mediana_ns} para todos los benches con ese label."""
    found = {}
    if not os.path.isdir(root):
        return found
    for dirpath, dirnames, _ in os.walk(root):
        if label not in dirnames:
            continue
        path = os.path.join(dirpath, label, "estimates.json")
        if not os.path.isfile(path):
            continue
        with open(path, encoding="utf-8") as fh:
            data = json.load(fh)
        median = data.get("median", {}).get("point_estimate")
        if not isinstance(median, (int, float)) or median <= 0:
            continue
        key = os.path.relpath(dirpath, root).replace(os.sep, "/")
        found[key] = float(median)
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--criterion-dir", default="target/criterion")
    parser.add_argument("--base", default="base", help="nombre del baseline del commit base")
    parser.add_argument("--head", default="head", help="nombre del baseline del head del PR")
    parser.add_argument("--threshold", type=float, default=10.0,
                        help="regresion porcentual que falla el gate (default: 10)")
    args = parser.parse_args()

    base = load_estimates(args.criterion_dir, args.base)
    head = load_estimates(args.criterion_dir, args.head)

    if not base or not head:
        print("FALLO: no hay estimaciones para alguno de los baselines "
              "(base=%d, head=%d benches)" % (len(base), len(head)), file=sys.stderr)
        return 1

    only_base = sorted(set(base) - set(head))
    only_head = sorted(set(head) - set(base))
    if only_base or only_head:
        print("FALLO: los conjuntos de benches no coinciden", file=sys.stderr)
        for key in only_base:
            print("  solo en base: %s" % key, file=sys.stderr)
        for key in only_head:
            print("  solo en head: %s" % key, file=sys.stderr)
        return 1

    failures = []
    print("%-28s %12s %12s %10s" % ("bench", "base (ns)", "head (ns)", "delta %"))
    for key in sorted(base):
        b, h = base[key], head[key]
        delta = (h - b) / b * 100.0
        print("%-28s %12.1f %12.1f %+9.2f" % (key, b, h, delta))
        if delta >= args.threshold:
            failures.append((key, delta))

    if failures:
        print("\nFALLO: regresion >= %.1f%% en %d bench(es):" % (args.threshold, len(failures)),
              file=sys.stderr)
        for key, delta in failures:
            print("  %s: %+.2f%%" % (key, delta), file=sys.stderr)
        return 1

    print("\nok: ninguna regresion >= %.1f%% (%d benches comparados)" %
          (args.threshold, len(base)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
