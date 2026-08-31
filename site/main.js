let wasmModule = null;
let fontsReady = false;
let monacoReady = false;
let editorInstance = null;

// 实时预览：内容变化后防抖自动渲染
const AUTO_RENDER_DELAY = 700;
let autoRenderTimer = null;
let suppressAutoRender = false; // 程序化 setValue（载入示例）不触发自动渲染
let pendingEditorValue = null; // Monaco 尚未就绪时暂存的编辑器内容
let currentObjectUrl = null; // PNG 预览的 Blob URL（切换/重建时释放）

// 与 liecharts 演示一致：注册 JetBrains Mono（等宽）与 Noto Sans CJK SC（CJK）。
// parley 按字族名精确解析；CJK 字体必须命中，否则中文会渲染成豆腐块。
const FONTS_TO_LOAD = [
    {
        name: 'JetBrains Mono',
        url: 'https://cdn.jsdelivr.net/gh/JetBrains/JetBrainsMono@v2.304/fonts/ttf/JetBrainsMono-Regular.ttf',
    },
    {
        name: 'Noto Sans CJK SC',
        aliases: ['Noto Sans SC'],
        url: 'https://fonts.gstatic.com/s/notosanssc/v26/k3kXo84MPvpLmixcA63oeALhL4iP-Q8.otf',
    },
];

let currentSvg = null;
let currentPngBytes = null;

// ── Monaco Editor ──────────────────────────────────────────

require.config({
    paths: { vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.52.0/min/vs' },
});

require(['vs/editor/editor.main'], function () {
    monacoReady = true;

    monaco.editor.defineTheme('liemermaid-light', {
        base: 'vs',
        inherit: true,
        rules: [
            { token: 'keyword.mmd', foreground: '#0f766e' },
            { token: 'string.mmd', foreground: '#15803d' },
            { token: 'comment.mmd', foreground: '#9fa2a7', fontStyle: 'italic' },
            { token: 'arrow.mmd', foreground: '#b91c1c' },
            { token: 'identifier.mmd', foreground: '#1f2329' },
        ],
        colors: {
            'editor.background': '#f9f9f9',
            'editor.foreground': '#1f2329',
            'editorLineNumber.foreground': '#9fa2a7',
            'editorCursor.foreground': '#0d9488',
            'editor.selectionBackground': '#0d948820',
            'editor.lineHighlightBackground': '#f4f4f5',
            'editorIndentGuide.background': '#e6e8ea',
            'editorIndentGuide.activeBackground': '#d1d3d6',
            'editorBracketMatch.background': '#0d948815',
            'editorBracketMatch.border': '#0d9488',
            'editorWidget.background': '#ffffff',
            'editorWidget.border': '#e6e8ea',
        },
    });

    registerMermaidLanguage();

    createEditor();
});

// 注册一个轻量 mermaid 语法高亮（Monaco 无内置 mermaid 语言）。
function registerMermaidLanguage() {
    const keywords = [
        'graph', 'flowchart', 'sequenceDiagram', 'classDiagram', 'stateDiagram-v2',
        'stateDiagram', 'erDiagram', 'gitGraph', 'timeline', 'subgraph', 'end',
        'participant', 'actor', 'note', 'loop', 'alt', 'opt', 'par', 'rect',
        'classDef', 'class', 'direction', 'activate', 'deactivate',
    ];
    monaco.languages.register({ id: 'mermaid' });
    monaco.languages.setMonarchTokensProvider('mermaid', {
        keywords: keywords,
        tokenizer: {
            root: [
                [/%%.*$/, 'comment.mmd'],
                [/#.*$/, 'comment.mmd'],
                [/\b(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram-v2|stateDiagram|erDiagram|gitGraph|timeline)\b/, 'keyword.mmd'],
                [/-->/, 'arrow.mmd'],
                [/-.->/, 'arrow.mmd'],
                [/==>/, 'arrow.mmd'],
                [/--x/, 'arrow.mmd'],
                [/->>/, 'arrow.mmd'],
                [/-->>/, 'arrow.mmd'],
                [/\b(subgraph|end|participant|actor|note|loop|alt|opt|par|rect|classDef|class|direction|activate|deactivate)\b/, 'keyword.mmd'],
                [/"[^"]*"/, 'string.mmd'],
                [/\[[^\]]*\]/, 'string.mmd'],
                [/\([^)]*\)/, 'string.mmd'],
                [/[{}[\]]/, 'arrow.mmd'],
                [/[A-Za-z_][\w]*/, 'identifier.mmd'],
            ],
        },
    });
}

function createEditor() {
    const container = document.getElementById('monacoEditor');
    if (!container) return;

    editorInstance = monaco.editor.create(container, {
        value: '',
        language: 'mermaid',
        theme: 'liemermaid-light',
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
        lineNumbers: 'on',
        minimap: { enabled: true, showSlider: 'mouseover' },
        automaticLayout: true,
        scrollBeyondLastLine: false,
        tabSize: 4,
        renderWhitespace: 'selection',
        bracketPairColorization: { enabled: true },
        wordWrap: 'on',
        guides: { indentation: true, bracketPairs: true },
        padding: { top: 12, bottom: 12 },
        smoothScrolling: true,
        cursorBlinking: 'smooth',
        cursorSmoothCaretAnimation: 'on',
    });

    // 应用 Monaco 就绪前暂存的示例内容（初始化竞态兜底）
    if (pendingEditorValue !== null) {
        const value = pendingEditorValue;
        pendingEditorValue = null;
        suppressAutoRender = true;
        editorInstance.setValue(value);
        suppressAutoRender = false;
        if (wasmModule && fontsReady) {
            generateChart();
        }
    }

    editorInstance.onDidChangeModelContent(scheduleAutoRender);
}

function scheduleAutoRender() {
    if (suppressAutoRender) return;
    clearTimeout(autoRenderTimer);
    autoRenderTimer = setTimeout(function () {
        if (wasmModule && fontsReady) {
            generateChart();
        }
    }, AUTO_RENDER_DELAY);
}

function getEditorValue() {
    return editorInstance ? editorInstance.getValue() : '';
}

function setEditorValue(value) {
    pendingEditorValue = value;
    if (editorInstance) {
        suppressAutoRender = true;
        editorInstance.setValue(value);
        suppressAutoRender = false;
    }
}

// ── WASM ──────────────────────────────────────────────────

async function initWasm() {
    try {
        wasmModule = await import('./pkg/liemermaid_site.js');
        await wasmModule.default();
        console.log('WASM module loaded');
    } catch (err) {
        console.error('WASM load failed:', err);
        showError('WASM module failed to load: ' + err.message);
        throw err;
    }
}

async function loadFonts() {
    const generateBtn = document.getElementById('generateBtn');
    let lastFontBytes = null;

    for (const font of FONTS_TO_LOAD) {
        try {
            console.log('Loading font:', font.name);
            const response = await fetch(font.url);
            if (!response.ok) {
                throw new Error('HTTP ' + response.status);
            }
            const arrayBuffer = await response.arrayBuffer();
            const bytes = new Uint8Array(arrayBuffer);

            lastFontBytes = { arrayBuffer: arrayBuffer, bytes: bytes };

            const familyNames = [font.name].concat(font.aliases || []);
            for (const fam of familyNames) {
                wasmModule.register_font_bytes(fam, bytes);
            }

            try {
                for (const fam of familyNames) {
                    const fontFace = new FontFace(fam, arrayBuffer);
                    const loadedFont = await fontFace.load();
                    document.fonts.add(loadedFont);
                }
                console.log('Font loaded:', font.name, (font.aliases || []).length ? '(+' + font.aliases.length + ' alias)' : '');
            } catch (browserErr) {
                console.warn('Browser font load failed:', font.name, browserErr);
            }
        } catch (err) {
            console.warn('Font load failed:', font.name, err);
        }
    }

    if (lastFontBytes) {
        try {
            wasmModule.register_font_sans_serif_bytes('Noto Sans CJK SC', lastFontBytes.bytes);
            const fontFaceSs = new FontFace('sans-serif', lastFontBytes.arrayBuffer);
            const loadedSsFont = await fontFaceSs.load();
            document.fonts.add(loadedSsFont);
        } catch (err) {
            console.warn('Fallback font failed:', err);
        }
    }

    fontsReady = true;
    if (generateBtn) {
        generateBtn.disabled = false;
    }
    console.log('Font loading done');
}

// ── Charts & Examples ─────────────────────────────────────

const chartExamples = {
    flowchart: 'examples/flowchart.mmd',
    sequence: 'examples/sequence.mmd',
    class: 'examples/class.mmd',
    state: 'examples/state.mmd',
    er: 'examples/er.mmd',
    pie: 'examples/pie.mmd',
    gitgraph: 'examples/gitgraph.mmd',
    timeline: 'examples/timeline.mmd',
};

// ── UI Helpers ────────────────────────────────────────────

function showError(message) {
    const errorPanel = document.getElementById('errorPanel');
    const errorMsg = errorPanel.querySelector('.error-message');
    errorMsg.textContent = message;
    errorPanel.classList.remove('hidden');
}

function hideError() {
    const errorPanel = document.getElementById('errorPanel');
    errorPanel.classList.add('hidden');
}

function showPlaceholder(text) {
    document.getElementById('chartContainer').innerHTML =
        '<div class="empty-state"><svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M9 21V9"/></svg><p>' + text + '</p></div>';
    currentSvg = null;
    currentPngBytes = null;
}

function setLoading(loading) {
    const btn = document.getElementById('generateBtn');
    const spinner = btn.querySelector('.btn-spinner');
    const text = btn.querySelector('.btn-text');
    if (loading) {
        spinner.classList.remove('hidden');
        text.textContent = 'Rendering...';
        btn.disabled = true;
    } else {
        spinner.classList.add('hidden');
        text.textContent = 'Render';
        btn.disabled = false;
    }
}

// ── Core Actions ──────────────────────────────────────────

async function loadExample() {
    const chartType = document.getElementById('chartType').value;
    const examplePath = chartExamples[chartType];

    if (!examplePath) {
        setEditorValue('');
        return;
    }

    try {
        const response = await fetch(examplePath);
        if (!response.ok) {
            throw new Error('HTTP ' + response.status);
        }
        const text = await response.text();
        setEditorValue(text);
        hideError();
        // 载入示例后立即渲染（实时预览）
        if (editorInstance && wasmModule && fontsReady) {
            generateChart();
        }
    } catch (err) {
        showError('Failed to load example: ' + err.message);
    }
}

async function generateChart() {
    if (!wasmModule) {
        showError('WASM module not ready');
        return;
    }
    if (!fontsReady) {
        showError('Fonts still loading');
        return;
    }

    const text = getEditorValue();
    if (!text.trim()) {
        showError('Please enter Mermaid source');
        return;
    }

    setLoading(true);

    try {
        const mode = document.getElementById('renderMode').value;
        const container = document.getElementById('chartContainer');

        if (mode === 'png') {
            // render_png 把 width/height 作为目标尺寸（内容放大到目标，提升分辨率）。
            const pngBytes = wasmModule.render_mermaid_png(text, 1400, 1000);
            currentPngBytes = pngBytes;
            currentSvg = null;

            if (currentObjectUrl) {
                URL.revokeObjectURL(currentObjectUrl);
            }
            const uint8 = new Uint8Array(pngBytes);
            const blob = new Blob([uint8], { type: 'image/png' });
            currentObjectUrl = URL.createObjectURL(blob);

            container.innerHTML = '<img src="' + currentObjectUrl + '" alt="Diagram" />';
        } else {
            if (currentObjectUrl) {
                URL.revokeObjectURL(currentObjectUrl);
                currentObjectUrl = null;
            }
            // render 的 width/height 是上限；传大值让内容按自然尺寸排版。
            const svg = wasmModule.render_mermaid(text, 2000, 2000);
            currentSvg = svg;
            currentPngBytes = null;
            container.innerHTML = svg;
        }

        hideError();
    } catch (err) {
        showError('Render error: ' + err);
    } finally {
        setLoading(false);
    }
}

function downloadChart() {
    const mode = document.getElementById('renderMode').value;
    if (mode === 'svg') {
        downloadSvg();
    } else if (mode === 'png') {
        downloadPng();
    }
}

function downloadSvg() {
    if (!currentSvg) {
        showError('Please render a diagram first');
        return;
    }

    const chartType = document.getElementById('chartType').value;
    const blob = new Blob([currentSvg], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'liemermaid_' + chartType + '.svg';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

function downloadPng() {
    if (!currentPngBytes) {
        showError('Please render a diagram in PNG mode first');
        return;
    }

    const chartType = document.getElementById('chartType').value;
    const uint8 = new Uint8Array(currentPngBytes);
    const blob = new Blob([uint8], { type: 'image/png' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'liemermaid_' + chartType + '.png';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

// ── Resize Handle ─────────────────────────────────────────

function initResizeHandle() {
    const handle = document.getElementById('resizeHandle');
    const editorPanel = document.querySelector('.panel-editor');
    let isDragging = false;

    handle.addEventListener('mousedown', (e) => {
        isDragging = true;
        handle.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
        if (!isDragging) return;
        const workspace = document.querySelector('.workspace');
        const rect = workspace.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const pct = (x / rect.width) * 100;
        editorPanel.style.flex = 'none';
        editorPanel.style.width = Math.max(20, Math.min(80, pct)) + '%';
    });

    document.addEventListener('mouseup', () => {
        if (isDragging) {
            isDragging = false;
            handle.classList.remove('dragging');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// ── Initialization ────────────────────────────────────────

document.addEventListener('DOMContentLoaded', function () {
    const chartTypeSelect = document.getElementById('chartType');
    const loadExampleBtn = document.getElementById('loadExample');
    const generateBtn = document.getElementById('generateBtn');
    const renderModeSelect = document.getElementById('renderMode');
    const downloadBtn = document.getElementById('downloadBtn');

    loadExampleBtn.addEventListener('click', loadExample);
    generateBtn.addEventListener('click', generateChart);
    downloadBtn.addEventListener('click', downloadChart);

    chartTypeSelect.addEventListener('change', function () {
        loadExample();
    });

    renderModeSelect.addEventListener('change', function () {
        const hasChart = currentSvg !== null || currentPngBytes !== null;
        if (hasChart) {
            generateChart();
        }
    });

    // Keyboard shortcut: Ctrl+Enter -> Render
    document.addEventListener('keydown', function (e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
            e.preventDefault();
            generateChart();
        }
    });

    initResizeHandle();

    initWasm()
        .then(function () {
            return loadFonts();
        })
        .then(function () {
            // loadExample 内部就绪后会立即渲染；Monaco 晚就绪时由 createEditor 的
            // pendingEditorValue 兜底路径渲染。
            return loadExample();
        })
        .catch(function (err) {
            console.error('Initialization failed:', err);
        });
});
