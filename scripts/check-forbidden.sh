#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

fail=0

scan() {
  local pattern=$1
  local reason=$2

  if grep -rnE --include='*.rs' "$pattern" src tests benches examples fuzz 2>/dev/null; then
    printf 'PROHIBIDO: %s\n' "$reason" >&2
    fail=1
  fi
}

scan '(//|/\*).*\b(TODO|FIXME|XXX|HACK)\b' 'marcadores de trabajo pendiente'
scan '\b(todo|unimplemented)!\s*[\(\[{]' 'todo!/unimplemented!'
scan 'let\s+_\s*(:[^=]+)?=' 'descarte con let _'
scan '\.ok\(\)\s*;' 'Result descartado con .ok();'
scan 'Err\(_\)' 'Err(_) ignora el error'
scan 'map_err\(\s*\|_' 'map_err descarta el error original'

if grep -rnE --include='*.rs' '#!?\[allow\(' src tests benches examples fuzz 2>/dev/null \
  | grep -v 'allow(unsafe_code)'; then
  printf 'PROHIBIDO: #[allow] (usar #[expect] con motivo y ADR)\n' >&2
  fail=1
fi

while IFS= read -r -d '' file; do
  lines="$(wc -l < "$file")"
  if (( lines > 200 )); then
    printf 'PROHIBIDO: %s tiene %s líneas; el máximo es 200\n' "$file" "$lines" >&2
    fail=1
  fi
done < <(find . -type f \( -name '*.rs' -o -name '*.sh' \) \
  -not -path './.git/*' -not -path './target/*' -print0)

exit "$fail"
