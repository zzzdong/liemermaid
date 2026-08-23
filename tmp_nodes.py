import re, sys

name = sys.argv[1]
path = f"tests/golden/dbg_{name}.svg"
s = open(path, encoding="utf-8").read()

# 提取每个 class="node" 的 g 块，尝试拿 label（文本或 title）
print(f"=== {name} nodes ===")
for i, m in enumerate(re.finditer(r'<g[^>]*class="node"[^>]*>(.*?)</g>', s, re.S)):
    block = m.group(1)
    rect = re.search(r'<rect[^>]*x="([0-9.\-]+)"[^>]*y="([0-9.\-]+)"', block)
    # label: <text> 内容或 <title>
    title = re.search(r'<title>(.*?)</title>', block)
    txts = re.findall(r'<text[^>]*>(.*?)</text>', block)
    label = (title.group(1) if title else (txts[0] if txts else "?"))
    print(f"  {label:>8} rect=({rect.group(1)},{rect.group(2)})" if rect else f"  {label}: no-rect")
