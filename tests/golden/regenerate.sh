#!/usr/bin/env bash
# 重新生成黄金样本并运行结构化对拍测试。
#
# 两种生成源：
#   1. (首选) 官方 mermaid-cli：需 npm install @mermaid-js/mermaid-cli（需网络）
#   2. (回退)  从 dagre 布局数据生成：仅需 tests/dagre_ref/layouts.json（无需网络）
#
# 用法:
#   bash tests/golden/regenerate.sh            # 优先 mermaid-cli，失败则回退 dagre
#   bash tests/golden/regenerate.sh --mermaid  # 强制用 mermaid-cli
#   bash tests/golden/regenerate.sh --dagre    # 强制用 dagre
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/tests/golden"

MODE="${1:-auto}"

generate_mermaid() {
    echo "==> 用官方 mermaid-cli 生成黄金样本..."
    if ! command -v npm >/dev/null 2>&1; then
        echo "!! npm 不可用，无法生成官方样本"
        return 1
    fi
    if [ ! -d node_modules ]; then
        echo "==> 安装 @mermaid-js/mermaid-cli（首次需下载，请稍候）"
        npm install
    fi
    # 若系统已装 Chromium，注入 PUPPETEER_EXECUTABLE_PATH，避免 mermaid-cli 下载浏览器
    for CHROME in /usr/bin/chromium /usr/bin/chromium-browser /usr/bin/google-chrome \
                  /usr/bin/google-chrome-stable /usr/bin/chrome /snap/bin/chromium; do
        if [ -x "$CHROME" ]; then
            export PUPPETEER_EXECUTABLE_PATH="$CHROME"
            echo "==> 使用系统 Chromium: $CHROME"
            break
        fi
    done
    node generate_golden.js
}

generate_dagre() {
    echo "==> 用 dagre 布局数据生成黄金样本（回退模式）..."
    if [ ! -f ../dagre_ref/layouts.json ]; then
        echo "!! 缺少 tests/dagre_ref/layouts.json；请先执行 cd tests/dagre_ref && node run.js"
        return 1
    fi
    node generate_golden_from_dagre.js
}

case "$MODE" in
    --mermaid) generate_mermaid ;;
    --dagre)   generate_dagre ;;
    auto)
        if generate_mermaid; then :; else generate_dagre; fi
        ;;
    *)
        echo "未知模式: $MODE（可用 --mermaid / --dagre / auto）"
        exit 1
        ;;
esac

echo "==> 运行 Rust 结构化对拍测试（与官方 mermaid-cli 对比）..."
cd "$ROOT"
cargo test --test official_compare_test
