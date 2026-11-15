const { maskTeXComments, isEscaped } = require('./core/tex-text');

const COMMAND_ID = 'contextIntellisense.openMathEditor';
const VIEW_ID = 'contextIntellisense.equationEditor';
const VIEW_CONTAINER_ID = 'contextIntellisenseEquations';
const HOVER_SETTING = 'hover.enabled';
const EDITOR_SETTING = 'editor.enabled';
const EQUATION_CONTEXT = 'contextIntellisense.inEquation';
const EDITOR_VIEW_CONTEXT = 'contextIntellisense.equationEditorTriggered';
const DEFAULT_ENVIRONMENTS = new Set([
  'formula', 'math', 'align', 'mathalignment', 'cases', 'matrix', 'pmatrix', 'bmatrix', 'vmatrix', 'Vmatrix',
  'gather', 'equation', 'multline', 'flalign', 'aligned', 'gathered', 'smallmatrix'
]);
const LATEX_ENVIRONMENTS = new Set([
  'align', 'align*', 'aligned', 'cases', 'equation', 'equation*', 'flalign', 'flalign*',
  'gather', 'gather*', 'gathered', 'matrix', 'multline', 'multline*', 'pmatrix', 'smallmatrix',
  'split', 'Vmatrix', 'vmatrix', 'bmatrix'
]);
const FORMULA_ENVIRONMENTS = new Set(['formula', 'math', 'equation', 'equation*', 'displaymath']);
let renderer;

function renderEquation(tex, display) {
  renderer ||= require('../media/context-equation-node/context_equation_wasm.js');
  return JSON.parse(renderer.renderEquation(tex, display));
}

module.exports = function registerMathEquationFeatures(vscode, context, getCache) {
  const selector = [
    { language: 'context-tex', scheme: 'file' },
    { language: 'context-internal-tex', scheme: 'file' }
  ];
  let editorView;
  let activeTarget;
  const documentSpanCache = new WeakMap();
  const renderCache = new Map();

  const mathConfiguration = () => vscode.workspace.getConfiguration('contextIntellisense.math');
  const editorEnabled = () => mathConfiguration().get(EDITOR_SETTING, true);

  function mathEnvironments() {
    const names = new Set(DEFAULT_ENVIRONMENTS);
    for (const [command, metadata] of getCache().commandMetadata) {
      if (metadata.category === 'mathematics' && metadata.type === 'environment' && command.startsWith('start')) {
        names.add(command.slice(5));
      }
    }
    return names;
  }

  function documentSpans(document) {
    const cached = documentSpanCache.get(document);
    if (cached?.version === document.version) return cached.spans;
    const spans = spansIn(document.getText());
    documentSpanCache.set(document, { version: document.version, spans });
    return spans;
  }

  function spanAt(spans, offset) {
    const enclosing = spans.filter(span => span.start <= offset && offset < span.end);
    return enclosing.filter(span => FORMULA_ENVIRONMENTS.has(span.environment))
      .sort((left, right) => left.end - left.start - (right.end - right.start))[0] ?? enclosing[0];
  }

  function updateEquationContext(editor = vscode.window.activeTextEditor, selection = editor?.selection) {
    const isTex = editor && ['context-tex', 'context-internal-tex'].includes(editor.document.languageId);
    const span = isTex && selection && spanAt(documentSpans(editor.document), editor.document.offsetAt(selection.active));
    return vscode.commands.executeCommand('setContext', EQUATION_CONTEXT, Boolean(span));
  }

  function spansIn(text) {
    const syntax = maskTeXComments(text);
    const names = mathEnvironments();
    const spans = [];
    const stack = [];
    const environmentRe = /\\(start|stop)([A-Za-z@:_-]+)|\\(begin|end)\s*\{([A-Za-z*]+)\}/g;
    let command;
    while ((command = environmentRe.exec(syntax)) !== null) {
      if (isEscaped(syntax, command.index)) continue;
      const isContext = Boolean(command[1]);
      const name = isContext ? command[2] : command[4];
      if (isContext && !names.has(name)) continue;
      if (!isContext && !LATEX_ENVIRONMENTS.has(name)) continue;
      const opening = isContext ? command[1] === 'start' : command[3] === 'begin';
      if (opening) {
        let bodyStart = environmentRe.lastIndex;
        if (isContext) {
          while (true) {
            while (/\s/.test(syntax[bodyStart] || '')) bodyStart += 1;
            if (syntax[bodyStart] !== '[') break;
            let depth = 0;
            do {
              if (syntax[bodyStart] === '[') depth += 1;
              else if (syntax[bodyStart] === ']') depth -= 1;
              bodyStart += 1;
            } while (bodyStart < syntax.length && depth > 0);
          }
        }
        stack.push({
          name,
          isContext,
          start: command.index,
          bodyStart,
          display: name.toLowerCase() !== 'math'
        });
      } else {
        const openIndex = stack.map(item => item.name === name && item.isContext === isContext).lastIndexOf(true);
        if (openIndex < 0) continue;
        const opening = stack.splice(openIndex, 1)[0];
        const body = text.slice(opening.bodyStart, command.index);
        const leading = body.length - body.trimStart().length;
        const trailing = body.trimEnd().length;
        if (trailing > leading) spans.push({
          start: opening.start,
          end: environmentRe.lastIndex,
          bodyStart: opening.bodyStart + leading,
          bodyEnd: opening.bodyStart + trailing,
          tex: body.trim(),
          display: opening.display,
          environment: name,
          isContext: opening.isContext
        });
      }
    }

    spans.sort((left, right) => left.start - right.start || right.end - left.end);
    const blocked = spans.map(span => [span.start, span.end]);
    let blockedIndex = 0;
    for (let i = 0; i < syntax.length; i += 1) {
      while (blockedIndex < blocked.length && blocked[blockedIndex][1] <= i) blockedIndex += 1;
      if (blockedIndex < blocked.length && blocked[blockedIndex][0] <= i) {
        i = blocked[blockedIndex][1] - 1;
        continue;
      }
      if (syntax[i] === '\\' && !isEscaped(syntax, i)) {
        const delimiter = syntax[i + 1] === '(' ? ['\\(', '\\)', false]
          : syntax[i + 1] === '[' ? ['\\[', '\\]', true]
            : undefined;
        if (delimiter) {
          const close = findClose(syntax, i + 2, delimiter[1]);
          if (close > i + 2) {
            spans.push(makeDelimitedSpan(text, i, close + 2, i + 2, close, delimiter[2], delimiter[0], delimiter[1]));
            i = close + 1;
            continue;
          }
        }
      }
      if (syntax[i] !== '$' || isEscaped(syntax, i)) continue;
      const delimiter = syntax[i + 1] === '$' ? '$$' : '$';
      const close = findClose(syntax, i + delimiter.length, delimiter);
      if (close < 0 || close === i + delimiter.length) continue;
      const end = close + delimiter.length;
      spans.push(makeDelimitedSpan(text, i, end, i + delimiter.length, close, delimiter === '$$', delimiter, delimiter));
      i = end - 1;
    }
    return spans.sort((left, right) => left.start - right.start || right.end - left.end);
  }

  function findClose(text, from, delimiter) {
    for (let i = from; i < text.length; i += 1) {
      if (delimiter === '$' && text[i] === '$' && text[i + 1] === '$') {
        i += 1;
        continue;
      }
      if (text.startsWith(delimiter, i) && !isEscaped(text, i)) return i;
    }
    return -1;
  }

  function makeDelimitedSpan(text, start, end, bodyStart, bodyEnd, display, open, close) {
    const body = text.slice(bodyStart, bodyEnd);
    const leading = body.length - body.trimStart().length;
    const trailing = body.trimEnd().length;
    return {
      start, end, bodyStart: bodyStart + leading, bodyEnd: bodyStart + trailing,
      tex: body.trim(), display, open, close, isContext: false
    };
  }

  function renderedSvg(span) {
    const kind = vscode.window.activeColorTheme.kind;
    const color = kind === vscode.ColorThemeKind.Dark || kind === vscode.ColorThemeKind.HighContrast
      ? '#e8e8e8' : '#111111';
    const key = `${kind}:${span.display}:${span.tex}`;
    let svg = renderCache.get(key);
    if (!svg) {
      svg = renderEquation(span.tex, span.display).svg;
      if (typeof svg !== 'string' || !svg.startsWith('<svg')) throw new Error('The equation renderer returned no SVG image.');
      if (renderCache.size >= 100) renderCache.delete(renderCache.keys().next().value);
      renderCache.set(key, svg);
    }
    const style = `color:${color};font-size:20px`;
    return svg.replace(/<svg\b[^>]*>/, root => root.includes(' style="')
      ? root.replace(/style="([^"]*)"/, (_, current) => `style="${style};${current}"`)
      : root.replace('<svg', `<svg style="${style}"`));
  }

  function hoverMarkdown(span, document, offset) {
    const markdown = new vscode.MarkdownString();
    markdown.isTrusted = { enabledCommands: [COMMAND_ID] };
    markdown.supportThemeIcons = true;
    markdown.supportHtml = true;
    try {
      const image = encodeURIComponent(renderedSvg(span)).replace(/[()]/g, char => `%${char.charCodeAt(0).toString(16)}`);
      markdown.appendMarkdown(`<img alt="Equation preview" src="data:image/svg+xml,${image}">\n\n`);
    } catch (error) {
      markdown.appendMarkdown('**Equation preview failed**\n\n');
      markdown.appendCodeblock(error instanceof Error ? error.message : String(error), 'text');
      markdown.appendMarkdown('\n');
    }
    if (editorEnabled()) {
      const args = encodeURIComponent(JSON.stringify([document.uri.toString(), offset]));
      markdown.appendMarkdown(`\n[ $(edit) Open visual equation editor](command:${COMMAND_ID}?${args})`);
    }
    return markdown;
  }

  function selectedTarget(editor, position) {
    const { document, selection } = editor;
    const text = document.getText();
    const offset = document.offsetAt(position);
    if (!selection.isEmpty) {
      const start = document.offsetAt(selection.start);
      const end = document.offsetAt(selection.end);
      const selectedSpan = spansIn(text.slice(start, end)).find(span => span.start === 0 && span.end === end - start);
      if (selectedSpan) {
        return {
          tex: selectedSpan.tex,
          display: selectedSpan.display,
          range: new vscode.Range(document.positionAt(start + selectedSpan.bodyStart), document.positionAt(start + selectedSpan.bodyEnd)),
          insertWrapper: false,
          version: document.version
        };
      }
      const enclosing = documentSpans(document).find(span => span.start <= start && span.end >= end);
      if (enclosing) return {
        tex: text.slice(start, end), display: enclosing.display,
        range: new vscode.Range(selection.start, selection.end), insertWrapper: false, version: document.version
      };
      return { tex: text.slice(start, end), display: true, range: new vscode.Range(selection.start, selection.end), insertWrapper: false, version: document.version };
    }
    const span = spanAt(documentSpans(document), offset);
    if (span) return {
      tex: span.tex,
      display: span.display,
      range: new vscode.Range(document.positionAt(span.bodyStart), document.positionAt(span.bodyEnd)),
      insertWrapper: false,
      version: document.version
    };
    return {
      tex: '', display: true,
      range: new vscode.Range(position, position),
      insertWrapper: true,
      version: document.version
    };
  }

  function editorHtml(webview, tex, display) {
    const themeKind = vscode.window.activeColorTheme.kind;
    const theme = themeKind === vscode.ColorThemeKind.Dark || themeKind === vscode.ColorThemeKind.HighContrast ? 'dark' : 'light';
    const script = webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, 'media', 'context-equation-editor.js'));
    const csp = [
      "default-src 'none'",
      `script-src ${webview.cspSource} 'wasm-unsafe-eval'`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `img-src ${webview.cspSource} data: blob:`,
      `font-src ${webview.cspSource} data:`,
      `connect-src ${webview.cspSource}`
    ].join('; ');
    const init = JSON.stringify({ tex, display, theme }).replace(/</g, '\\u003c');
    return `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}"><meta name="viewport" content="width=device-width,initial-scale=1">
<style>html,body{height:100%;margin:0}body{color-scheme:${theme};background:var(--vscode-editor-background)}</style>
</head><body><script id="init" type="application/json">${init}</script><script type="module" src="${script}"></script></body></html>`;
  }

  function editorOptions(target) {
    return { tex: target?.tex || '', display: target?.display ?? true };
  }

  const editorViewProvider = {
    resolveWebviewView(view) {
      editorView = view;
      view.title = 'Equation';
      view.webview.options = {
        enableScripts: true,
        localResourceRoots: [context.extensionUri]
      };
      const { tex, display } = editorOptions(activeTarget);
      view.webview.html = editorHtml(view.webview, tex, display);
      view.onDidDispose(() => { if (editorView === view) editorView = undefined; });
      view.webview.onDidReceiveMessage(async message => {
        if (message?.type === 'reset') {
          view.webview.postMessage({ type: 'set', tex: activeTarget?.tex || '' });
          return;
        }
        if (message?.type !== 'apply' || typeof message.tex !== 'string') return;
        const equation = message.tex;
        const target = activeTarget;
        const document = target?.document;
        const prefix = target?.insertWrapper ? '\\startformula\n' : '';
        const text = !document || !target ? `\\startformula\n${equation}\n\\stopformula`
          : target.insertWrapper ? `${prefix}${equation}\n\\stopformula` : equation;
        if (!document || !target) {
          await vscode.env.clipboard.writeText(text);
          vscode.window.setStatusBarMessage('ConTeXt IntelliSense: equation copied', 3000);
          return;
        }
        if (document.version !== target.version) {
          vscode.window.showWarningMessage('The TeX document changed while the equation editor was open. Reopen the editor to avoid replacing newer text.');
          return;
        }
        const edit = new vscode.WorkspaceEdit();
        edit.replace(document.uri, target.range, text);
        if (!await vscode.workspace.applyEdit(edit)) {
          vscode.window.showErrorMessage('The equation could not be inserted into the document.');
          return;
        }
        const start = document.offsetAt(target.range.start) + prefix.length;
        target.range = new vscode.Range(document.positionAt(start), document.positionAt(start + equation.length));
        target.tex = equation;
        target.insertWrapper = false;
        target.version = document.version;
        view.webview.postMessage({ type: 'set', tex: equation });
        vscode.window.setStatusBarMessage('ConTeXt IntelliSense: equation updated', 3000);
      });
    }
  };

  function makeTarget(editor, position) {
    const target = selectedTarget(editor, position);
    target.document = editor.document;
    return target;
  }

  function editorFromCommandArgs(args) {
    const uri = args.find(value => value && typeof value.scheme === 'string' && typeof value.path === 'string');
    const editors = [vscode.window.activeTextEditor, ...vscode.window.visibleTextEditors];
    return uri
      ? editors.find(editor => editor?.document.uri.toString() === uri.toString())
      : vscode.window.activeTextEditor;
  }

  const hoverProvider = vscode.languages.registerHoverProvider(selector, {
    provideHover(document, position) {
      if (!mathConfiguration().get(HOVER_SETTING, true)) return undefined;
      const offset = document.offsetAt(position);
      const span = spanAt(documentSpans(document), offset);
      if (!span) return undefined;
      return new vscode.Hover(
        hoverMarkdown(span, document, offset),
        new vscode.Range(document.positionAt(span.start), document.positionAt(span.end))
      );
    }
  });
  updateEquationContext();
  void vscode.commands.executeCommand('setContext', EDITOR_VIEW_CONTEXT, false);
  context.subscriptions.push(
    hoverProvider,
    vscode.window.onDidChangeTextEditorSelection(event => {
      if (event.textEditor === vscode.window.activeTextEditor) void updateEquationContext(event.textEditor, event.selections[0]);
    }),
    vscode.window.onDidChangeActiveTextEditor(editor => { void updateEquationContext(editor); }),
    vscode.workspace.onDidChangeTextDocument(event => {
      if (event.document === vscode.window.activeTextEditor?.document) void updateEquationContext();
    }),
    { dispose: () => { void vscode.commands.executeCommand('setContext', EQUATION_CONTEXT, false); } },
    vscode.window.registerWebviewViewProvider(VIEW_ID, editorViewProvider, { webviewOptions: { retainContextWhenHidden: true } }),
    vscode.commands.registerCommand(COMMAND_ID, async (...args) => {
      if (!editorEnabled()) return;
      const hoverInvocation = typeof args[0] === 'string' && Number.isInteger(args[1]);
      let editor;
      if (hoverInvocation) {
        const [uri, offset] = args;
        const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
        const position = document.positionAt(offset);
        editor = { document, selection: new vscode.Selection(position, position) };
        activeTarget = makeTarget(editor, position);
      } else {
        editor = editorFromCommandArgs(args);
        activeTarget = editor
          ? makeTarget(editor, editor.selection.isEmpty ? editor.selection.active : editor.selection.start)
          : undefined;
      }
      await vscode.commands.executeCommand('setContext', EDITOR_VIEW_CONTEXT, true);
      const existingView = editorView;
      await vscode.commands.executeCommand(`workbench.view.extension.${VIEW_CONTAINER_ID}`);
      if (editorView) {
        editorView.show(false);
        if (existingView) editorView.webview.postMessage({ type: 'load', ...editorOptions(activeTarget) });
      }
    })
  );
};
