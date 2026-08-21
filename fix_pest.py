p = 'src/grammar/mermaid.pest'
lines = open(p).read().split('\n')
BS = chr(92)  # backslash
# desired: node_shape_parallelogram_alt = ${ "[\\" ~ (!"\\]" ~ ANY)* ~ "\\]" }
para = 'node_shape_parallelogram_alt = ${ "' + '[' + BS + BS + '" ~ (!"' + BS + BS + ']" ~ ANY)* ~ "' + BS + BS + ']" }'
trap = 'node_shape_trapezoid_alt = ${ "' + '[' + BS + BS + '" ~ (!"/]" ~ ANY)* ~ "/]" }'
for i, ln in enumerate(lines):
    if ln.startswith('node_shape_parallelogram_alt = '):
        lines[i] = para
    elif ln.startswith('node_shape_trapezoid_alt = '):
        lines[i] = trap
open(p, 'w').write('\n'.join(lines))
for i, ln in enumerate(lines):
    if 'parallelogram_alt' in ln or 'trapezoid_alt' in ln:
        print(i + 1, repr(ln))
