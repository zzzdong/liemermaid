// 备用生成器：从 dagre 布局数据（tests/dagre_ref/layouts.json）生成"黄金样本" SVG。
//
// 适用场景：
//   1. 环境中没有 npm / 无法安装 @mermaid-js/mermaid-cli（如 CI 无网络）
//   2. 需要快速自举 golden 样本，让 Rust 对拍测试先跑起来
//
// dagre 正是 mermaid flowchart 的官方布局引擎，因此 dagre 输出的节点坐标
// 即为 mermaid 布局的"标准答案"。本脚本将其转成与官方 mermaid SVG 同构的
// 节点结构（`<g id="A" transform="translate(cx,cy)">`），供 Rust 结构化解析。
//
// 当网络可用、能安装 mermaid-cli 时，请优先用 generate_golden.js 生成真正的
// 官方 SVG（含文本测宽），以获得更精确的对拍。
//
// 运行：
//   node generate_golden_from_dagre.js

'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = __dirname;
const GOLDEN_DIR = path.join(ROOT, 'golden');
const DAGRE_FIXTURE = path.join(ROOT, '..', 'dagre_ref', 'layouts.json');
const CASES_DIR = path.join(ROOT, 'cases');

function genSvg(caseName, nodes) {
  // 计算画布尺寸（内容外包 + margin）
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const n of Object.values(nodes)) {
    minX = Math.min(minX, n.x - n.w / 2);
    minY = Math.min(minY, n.y - n.h / 2);
    maxX = Math.max(maxX, n.x + n.w / 2);
    maxY = Math.max(maxY, n.y + n.h / 2);
  }
  const margin = 40;
  const width = maxX - minX + 2 * margin;
  const height = maxY - minY + 2 * margin;
  const offX = margin - minX;
  const offY = margin - minY;

  let out = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">\n`;
  out += `<g class="output">\n  <g class="nodes">\n`;
  for (const [id, n] of Object.entries(nodes)) {
    const cx = n.x + offX;
    const cy = n.y + offY;
    const x = cx - n.w / 2;
    const y = cy - n.h / 2;
    out += `    <g class="node default" id="${id}" transform="translate(${cx}, ${cy})">\n`;
    out += `      <rect x="${x}" y="${y}" width="${n.w}" height="${n.h}" rx="6" ry="6" class="basic label-container"/>\n`;
    out += `      <g class="label"><g class="nodeLabel"><text>${id}</text></g></g>\n`;
    out += `    </g>\n`;
  }
  out += `  </g>\n</g>\n</svg>\n`;
  return out;
}

const fixture = JSON.parse(fs.readFileSync(DAGRE_FIXTURE, 'utf8'));
fs.mkdirSync(GOLDEN_DIR, { recursive: true });

// 读取 catalog 的用例来源
const catalog = JSON.parse(fs.readFileSync(path.join(CASES_DIR, 'catalog.json'), 'utf8'));
const bySource = {};
for (const c of catalog.cases) {
  bySource[`${c.type}__${c.name}`] = c;
}

let written = 0;
for (const c of fixture.cases) {
  // dagre case 名 -> golden 用例名（chain/diamond/cycle/long_edge/split/cross/diamond_lr/diamond_bt/diamond_rl）
  const caseName = c.name;
  if (!bySource[`flowchart__${caseName}`]) {
    // 无对应 catalog 用例则跳过（例如 dagre 特有的 name）
    continue;
  }
  const outFile = path.join(GOLDEN_DIR, `flowchart__${caseName}.svg`);
  const svg = genSvg(caseName, c.nodes);
  fs.writeFileSync(outFile, svg, 'utf8');
  written += 1;
  console.log(`  ✓ flowchart__${caseName} (${Object.keys(c.nodes).length} nodes)`);
}

console.log(`\n=== dagre golden generation: ${written} cases written to ${path.relative(ROOT, GOLDEN_DIR)} ===`);
if (!written) {
  console.error('No golden files generated. Check that tests/dagre_ref/layouts.json exists and run.js was executed.');
  process.exit(1);
}
