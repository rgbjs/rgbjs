#!/usr/bin/env python3
"""Extracts invoice test vectors from upstream Rust test code.

  python3 -I scripts/extract-vectors.py <line> <crate@version> <file.rs> ... > test/vectors/<line>-upstream-invoices.json

Picks up `let <var> = "<rgb:|contract:...>";` literals and classifies the
statement that follows: `.unwrap()` means the upstream test expects it to
parse, `Err(...)`/`unwrap_err` means it must be rejected. `roundtrip` is set
when the same block asserts that printing it gives back the literal.
"""
import json
import re
import sys

LIT = re.compile(r'let\s+(\w+)\s*=\s*"((?:[^"\\]|\\.)*)"\s*;', re.S)


def unescape(s):
    s = re.sub(r'\\\n\s*', '', s)  # Rust string continuation
    return s.replace('\\"', '"').replace('\\\\', '\\')


line, source, *files = sys.argv[1:]
out = []
for path in files:
    text = open(path).read()
    rel = path.rsplit('/src/', 1)[-1]
    matches = list(LIT.finditer(text))
    for i, m in enumerate(matches):
        var, value = m.group(1), unescape(m.group(2))
        if not value.startswith(('rgb:', 'contract:', 'bad:')) and ':utxob:' not in value:
            continue
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        gap = text.find('\n\n', m.end())  # upstream separates cases with blank lines
        if gap != -1:
            end = min(end, gap)
        block = text[m.end():end]
        lineno = text[:m.start()].count('\n') + 1
        err = re.search(r'Err\((?:\w+::)*(\w+)', block)
        if err or 'unwrap_err' in block:
            expect, kind = 'error', err.group(1) if err else None
        elif '.unwrap()' in block:
            expect, kind = 'ok', None
        else:
            continue
        roundtrip = bool(re.search(rf'to_string\(\),\s*{var}\b|{var},\s*\w+\.to_string\(\)', block))
        v = {'input': value, 'expect': expect, 'source': f'{source} src/{rel}:{lineno}'}
        if kind:
            v['error'] = kind
        if expect == 'ok':
            v['roundtrip'] = roundtrip
        out.append(v)
json.dump({'line': line, 'vectors': out}, sys.stdout, indent=2)
print()
