#!/usr/bin/env bash
# 重新生成黄金样本并运行结构化对拍测试。
#
# 生成源：官方 mermaid-cli（需 npm install @mermaid-js/mermaid-cli，需网络）
#
# 用法:
#   bash tests/golden/regenerate.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/tests/golden"

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

generate_mermaid

echo "==> 运行 Rust 结构化对拍测试（与官方 mermaid-cli 对比）..."
cd "$ROOT"
cargo test --test official_compare_test
