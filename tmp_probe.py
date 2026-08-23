import re

def probe_official(path):
    c = open(path, encoding='utf-8').read()
    print('=== OFFICIAL', path.split('/')[-1], '===')
    nodes = re.findall(r'<g[^>]*class="node[^"]*"[^>]*>', c)
    print('g.node count:', len(nodes))
    # show first 2 node blocks (transform + text)
    for m in list(re.finditer(r'<g[^>]*class="node[^"]*"[^>]*>(.*?)</g>', c, re.S))[:2]:
        block = m.group(0)
        trans = re.search(r'transform="translate\(([^)]*)\)"', block)
        # text inside foreignObject div/p
        txt = re.findall(r'<(?:p|div)[^>]*>([^<]+)<', block)
        rect = re.search(r'<rect[^>]*width="([^"]*)"[^>]*height="([^"]*)"', block)
        print('  transform=', trans.group(1) if trans else 'NONE',
              'rect_wh=', (rect.group(1), rect.group(2)) if rect else 'NONE',
              'texts=', txt[:3])
    # edge paths
    edgepaths = re.findall(r'class="edgePaths"', c)
    print('has edgePaths group:', len(edgepaths) > 0)
    # sample one edge path d
    d = re.search(r'<path[^>]*class="[^"]*path[^"]*"[^>]*d="([^"]*)"', c)
    print('sample edge d:', (d.group(1)[:80] if d else 'NONE'))

def probe_liem(path_label, svg):
    print('=== LIEMERMAID', path_label, '===')
    nodes = re.findall(r'class="node"', svg)
    print('class=node count:', len(nodes))
    # show context around first 2 node classes
    for m in list(re.finditer(r'class="node"', svg))[:2]:
        s = max(0, m.start()-60); e = min(len(svg), m.end()+200)
        frag = svg[s:e].replace('\n',' ')
        print('  frag:', frag)
    # text elements
    texts = re.findall(r'<text[^>]*>([^<]*)</text>', svg)
    print('text contents sample:', texts[:5])
    # edges
    edges = re.findall(r'class="edge"', svg)
    print('class=edge count:', len(edges))

# official
probe_official('tests/golden/golden/flowchart__chain.svg')

# liemermaid from report.html (first svg after the key is liemermaid)
c = open('tests/golden/report.html', encoding='utf-8').read()
i = c.find('flowchart__chain')
seg = c[i:i+60000]
si = seg.find('<svg'); ei = seg.find('</svg>', si)
svg_l = seg[si:ei+6]
probe_liem('flowchart__chain', svg_l)
