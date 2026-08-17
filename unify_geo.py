import os, re

SRC = "d:/code/rust/liemermaid/src"
KURBO_RE = re.compile(r'^\s*use\s+vello_cpu::kurbo::(.*?);\s*$')

def parse_parts(inner):
    inner = inner.strip()
    # inner like "Point" or "{ BezPath, Point, Rect }"
    if inner.startswith('{') and inner.endswith('}'):
        inner = inner[1:-1]
    parts = [p.strip() for p in inner.split(',') if p.strip()]
    return parts

for root, _, files in os.walk(SRC):
    for f in files:
        if not f.endswith('.rs'):
            continue
        p = os.path.join(root, f)
        lines = open(p, encoding='utf-8').read().split('\n')
        out = []
        changed = False
        for ln in lines:
            m = KURBO_RE.match(ln)
            if m:
                parts = parse_parts(m.group(1))
                has_pt = 'Point' in parts
                has_rect = 'Rect' in parts
                has_bz = 'BezPath' in parts
                has_ps = 'PathSeg' in parts
                indent = ln[:len(ln) - len(ln.lstrip())]
                if has_pt or has_rect:
                    combo = []
                    if has_pt: combo.append('Point')
                    if has_rect: combo.append('Rect')
                    out.append(f"{indent}use lievisual::geometry::{{{', '.join(combo)}}};")
                    changed = True
                if has_bz:
                    out.append(f"{indent}use vello_cpu::kurbo::BezPath;")
                    changed = True
                if has_ps:
                    out.append(f"{indent}use vello_cpu::kurbo::PathSeg;")
                    changed = True
                # 若只有 Point/Rect 被抽走且无其它，则这行 kurbo 已完全替换；无残留
                continue
            # 限定调用 vello_cpu::kurbo::Point::new / Rect::new -> Point::new / Rect::new
            new = ln.replace('vello_cpu::kurbo::Point::new', 'Point::new')
            new = new.replace('vello_cpu::kurbo::Rect::new', 'Rect::new')
            if new != ln:
                changed = True
            out.append(new)
        if changed:
            open(p, 'w', encoding='utf-8').write('\n'.join(out))
            print("unified:", p)
