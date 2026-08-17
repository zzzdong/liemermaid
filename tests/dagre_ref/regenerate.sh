#!/usr/bin/env bash
# 重新生成 dagre fixture 并跑 liemermaid 官方对拍测试。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/tests/dagre_ref"
node run.js
cd "$ROOT"
cargo test --test official_compare_test
