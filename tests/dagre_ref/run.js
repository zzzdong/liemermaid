// 用 @dagrejs/dagre 生成"官方"布局 fixture，供 Rust 集成测试做结构化对拍。
//
// 输出: layouts.json
//   {
//     "cases": [
//       {
//         "name": "...",
//         "rankdir": "TB",
//         "nodes": { "A": { "x": 12.3, "y": 45.6, "w": 100, "h": 40 }, ... },
//         "edges": [ { "from": "A", "to": "B", "points": [ {"x":..,"y":..}, ... ] }, ... ]
//       }
//     ]
//   }
//
// 坐标说明: dagre 输出的是节点左上角 (x,y) + width/height。
// 为与 liemermaid 的 SugiyamaResult.positions(节点中心) 对齐，
// 这里统一转成"中心坐标": cx = x + w/2, cy = y + h/2。

const dagre = require('@dagrejs/dagre');

// 统一节点尺寸，消除 mermaid 文字测宽差异（对拍只关心布局算法，不关心皮肤）
const NODE_W = 100;
const NODE_H = 40;

function runCase(name, rankdir, edges, nodes) {
  const g = new dagre.graphlib.Graph({ multigraph: true });
  g.setGraph({
    rankdir,
    nodesep: 50,   // 对应 liemermaid node_gap
    ranksep: 60,   // 对应 liemermaid layer_gap
    marginx: 40,
    marginy: 40,
  });
  g.setDefaultEdgeLabel(() => ({}));

  const nodeIds = nodes || Array.from(new Set(edges.flatMap((e) => [e[0], e[1]])));
  for (const id of nodeIds) {
    g.setNode(id, { width: NODE_W, height: NODE_H });
  }
  edges.forEach((e, i) => g.setEdge(e[0], e[1], {}, `e${i}`));

  dagre.layout(g);

  const outNodes = {};
  for (const id of g.nodes()) {
    const n = g.node(id);
    outNodes[id] = {
      x: n.x + n.width / 2,   // 中心坐标
      y: n.y + n.height / 2,
      w: n.width,
      h: n.height,
    };
  }
  const outEdges = edges.map((e, i) => {
    const e0 = g.edge(e[0], e[1], `e${i}`);
    return {
      from: e[0],
      to: e[1],
      points: (e0.points || []).map((p) => ({ x: p.x, y: p.y })),
    };
  });

  return { name, rankdir, nodes: outNodes, edges: outEdges };
}

const cases = [];

// 1. 简单链 A->B->C
cases.push(runCase('chain', 'TB', [['A', 'B'], ['B', 'C']]));

// 2. 菱形 A->B, A->C, B->D, C->D
cases.push(runCase('diamond', 'TB', [['A', 'B'], ['A', 'C'], ['B', 'D'], ['C', 'D']]));

// 3. 回边环 A->B<->C->D  (B<->C 构成环)
cases.push(runCase('cycle', 'TB', [['A', 'B'], ['B', 'C'], ['C', 'B'], ['C', 'D']]));

// 4. 长边 A->B->C->D 且 A->D (应拆 dummy)
cases.push(runCase('long_edge', 'TB', [['A', 'B'], ['B', 'C'], ['C', 'D'], ['A', 'D']]));

// 5. 同层分裂 A->B, A->C (B,C 同层)
cases.push(runCase('split', 'TB', [['A', 'B'], ['A', 'C']]));

// 6. 更多交叉 A->B, A->C, A->D, B->E, C->E, D->E
cases.push(runCase('cross', 'TB',
  [['A', 'B'], ['A', 'C'], ['A', 'D'], ['B', 'E'], ['C', 'E'], ['D', 'E']]));

// 7. LR 方向 (验证 rankdir 变换)
cases.push(runCase('diamond_lr', 'LR', [['A', 'B'], ['A', 'C'], ['B', 'D'], ['C', 'D']]));

// 8. BT 方向
cases.push(runCase('diamond_bt', 'BT', [['A', 'B'], ['A', 'C'], ['B', 'D'], ['C', 'D']]));

// 9. RL 方向
cases.push(runCase('diamond_rl', 'RL', [['A', 'B'], ['A', 'C'], ['B', 'D'], ['C', 'D']]));

const out = { cases };
require('fs').writeFileSync(
  require('path').join(__dirname, 'layouts.json'),
  JSON.stringify(out, null, 2)
);
console.log('wrote layouts.json with', cases.length, 'cases');
