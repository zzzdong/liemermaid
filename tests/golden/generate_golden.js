// 用官方 @mermaid-js/mermaid-cli 为每个测试用例生成"黄金样本" SVG。
//
// 输入:  ./cases/catalog.json  （描述用例 + 指向 .mmd 源码文件）
//        ./cases/{type}__{name}.mmd
// 输出:  ./golden/{type}__{name}.svg   （官方 mermaid-cli 渲染的标准输出）
//
// 运行:
//   cd tests/golden && npm install && node generate_golden.js
//
// 生成的 SVG 作为"黄金标准"存入仓库，供 Rust 测试做结构化对比
// （见 tests/golden_snapshot_test.rs）。
//
// 说明：mermaid-cli 的 SVG 会包含外部字体 URL 与主题相关元数据，
// 这些不影响布局结构。Rust 侧只做结构化对比（节点矩形位置/尺寸、边路径），
// 不比对文本/样式细节。

'use strict';

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT = __dirname;
const CASES_DIR = path.join(ROOT, 'cases');
const GOLDEN_DIR = path.join(ROOT, 'golden');
const OURS_DIR = path.join(ROOT, 'ours');

// --- 工具函数 ---

function cleanSvg(svg) {
  // mermaid-cli 输出的 SVG 里，节点元素带 classes 与 transform。
  // 我们保留标签结构以便 Rust 侧提取节点矩形 / 边路径。
  // 这里只做最少的归一化：
  //   1. 去除 xml 声明头（可选）
  //   2. 其他原样保留
  return svg.replace(/^<\?xml[^>]*\?>\s*/i, '');
}

// --- 系统 Chromium 检测 ---
//
// mermaid-cli 依赖 puppeteer，默认需要下载 chrome-headless-shell（大、慢、且常被
// 防火墙/离线环境拦截）。若系统已安装 chromium/chrome（如 `pacman -S chromium`），
// 我们通过 PUPPETEER_EXECUTABLE_PATH 让它直接用系统浏览器，无需下载。
const CHROMIUM_CANDIDATES = [
  process.env.PUPPETEER_EXECUTABLE_PATH,
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/usr/bin/chrome',
  '/snap/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
].filter(Boolean);

function detectChromium() {
  for (const c of CHROMIUM_CANDIDATES) {
    try {
      if (fs.existsSync(c)) return c;
    } catch (_) {
      /* ignore */
    }
  }
  return null;
}

const CHROMIUM = detectChromium();

if (CHROMIUM) {
  console.log(`==> 使用系统 Chromium: ${CHROMIUM}`);
} else {
  console.log('==> 未找到系统 Chromium，mmdc 将使用 puppeteer 自带浏览器（需先安装 chrome-headless-shell）');
}

// --- 主流程 ---

const catalog = JSON.parse(fs.readFileSync(path.join(CASES_DIR, 'catalog.json'), 'utf8'));

fs.mkdirSync(GOLDEN_DIR, { recursive: true });
fs.mkdirSync(OURS_DIR, { recursive: true });

let generated = 0;
let failed = [];

for (const c of catalog.cases) {
  const srcFile = path.join(CASES_DIR, c.source);
  const outFile = path.join(GOLDEN_DIR, `${c.type}__${c.name}.svg`);
  const tmpPng = path.join(GOLDEN_DIR, `.tmp_${c.type}__${c.name}.png`);

  if (!fs.existsSync(srcFile)) {
    failed.push(`${c.type}__${c.name}: missing source ${c.source}`);
    console.error(`  ✗ ${c.type}__${c.name}: source missing`);
    continue;
  }

  try {
    // mmdc 参数: -i 输入 -o 输出 -b 背景透明 -w/-H 画布尺寸
    const cmd = [
      'npx mmdc',
      `-i "${srcFile}"`,
      `-o "${outFile}"`,
      '-b transparent',
      `-w ${c.width}`,
      `-H ${c.height}`,
      '--pdfFit',
    ].join(' ');

    // 若找到系统 Chromium，注入 PUPPETEER_EXECUTABLE_PATH，让 mmdc 直接用系统浏览器。
    const env = CHROMIUM ? { ...process.env, PUPPETEER_EXECUTABLE_PATH: CHROMIUM } : process.env;

    // mermaid-cli 需要临时输出路径有正确的扩展名；SVG 输出即可。
    // 用 execSync 保证同步，失败立即抛出。
    execSync(cmd, { cwd: ROOT, stdio: 'inherit', env });

    // 清理临时 PNG（mmdc 会额外产出同名 .png）
    if (fs.existsSync(tmpPng)) fs.unlinkSync(tmpPng);

    // 归一化后重写
    if (fs.existsSync(outFile)) {
      let raw = fs.readFileSync(outFile, 'utf8');
      raw = cleanSvg(raw);
      fs.writeFileSync(outFile, raw, 'utf8');
    }

    generated += 1;
    console.log(`  ✓ ${c.type}__${c.name} -> ${path.relative(ROOT, outFile)}`);
  } catch (e) {
    failed.push(`${c.type}__${c.name}: ${e.message}`);
    console.error(`  ✗ ${c.type}__${c.name}: ${e.message.split('\n')[0]}`);
  }
}

console.log(`\n=== golden generation: ${generated}/${catalog.cases.length} OK ===`);
if (failed.length) {
  console.error('\nFailures:');
  failed.forEach((f) => console.error('  ' + f));
  process.exit(1);
}
