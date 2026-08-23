import re, sys

def nodes(path):
    try:
        s = open(path, encoding="utf-8").read()
    except FileNotFoundError:
        return {}
    out = {}
    # 官方 mermaid: <g class="node ..." ... transform="translate(x,y)"><...<p class="nodeLabel">LABEL</p>...
    for m in re.finditer(r'<g[^>]*class="([^"]*node[^"]*)"[^>]*transform="translate\(([0-9.\-]+),([0-9.\-]+)\)"', s):
        cls, x, y = m.group(1), float(m.group(2)), float(m.group(3))
        # label: 找最近 nodeLabel
        seg = s[m.start(): m.start()+2000]
        lab = re.search(r'class="nodeLabel"[^>]*>(.*?)</', seg, re.S)
        label = re.sub(r'<[^>]+>', '', lab.group(1)).strip() if lab else cls
        out[label] = (x, y)
    return out

name = sys.argv[1]
ours = nodes(f"tests/golden/dbg_{name}.svg")
gold = nodes(f"tests/golden/golden/{name}.svg")
print(f"=== {name} ===")
print("label | liemermaid(x,y) | official(x,y)")
for k in sorted(set(ours)|set(gold)):
    o = ours.get(k, "  -  ")
    g = gold.get(k, "  -  ")
    print(f"  {k:>8} | {str(o):>14} | {str(g):>14}")
