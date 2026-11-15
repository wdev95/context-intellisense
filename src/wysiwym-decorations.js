const BUILTIN_STYLES = new Map();
const addStyle = (names, style) => names.split(' ').forEach(name => BUILTIN_STYLES.set(name, style));
addStyle('bold bf bfseries textbf', { bold: true });
addStyle('em emph ems textit italic it textsl sl slanted', { italic: true });
addStyle('bi bs bolditalic italicbold boldslanted slantedbold', { bold: true, italic: true });

function mergeStyles(...styles) {
  return styles.reduce((result, style) => ({ ...result, ...style }), {});
}

function inferCommandStyle(name) {
  const normalized = String(name || '').toLowerCase();
  return BUILTIN_STYLES.get(normalized)
    || (/^bf(?:xx|x|[abcd])?$/.test(normalized) ? { bold: true } : undefined)
    || (/^(?:it|sl|bi|bs)(?:xx|x|[abcd])?$/.test(normalized)
      ? { bold: /^(?:bi|bs)/.test(normalized), italic: true }
      : undefined);
}

function extractSetting(settings, name) {
  for (const setting of settings) {
    const match = setting.match(new RegExp(`(?:^|,)\\s*${name}\\s*=\\s*(?:\\{([^{}]*)\\}|([^,\\]]+))`, 'i'));
    const value = (match?.[1] || match?.[2])?.trim();
    if (value) return value;
  }
  return '';
}

function createWysiwymDecorations({ vscode, isContextTexDocument, getCache, findMatchingDelimiter, maskTeXComments, loadRules }) {
  const decorations = new Map();
  let rules;

  function addRange(ranges, start, end, style) {
    if (end > start && Object.keys(style).length) ranges.push({ start, end, style });
  }

  function parseDocument(document) {
    const text = document.getText();
    const syntax = maskTeXComments(text);
    const structureRanges = [];
    const commandRe = /\\([A-Za-z@]+)/g;
    let command;
    while ((command = commandRe.exec(syntax)) !== null) {
      const metadata = getCache().commandMetadata.get(command[1]);
      if (metadata?.category !== 'structure' || !Number.isInteger(metadata.structureRank)) continue;
      let cursor = commandRe.lastIndex;
      let foundTitle = false;
      let firstBracket = true;
      for (let count = 0; count < 8; count += 1) {
        while (/\s/.test(syntax[cursor] || '')) cursor += 1;
        const open = syntax[cursor];
        if (open !== '[' && open !== '{') break;
        const close = findMatchingDelimiter(syntax, cursor, open, open === '[' ? ']' : '}');
        if (open === '[') {
          const bodyStart = cursor + 1;
          const body = syntax.slice(bodyStart, close);
          const title = findTitleAssignment(body, bodyStart, findMatchingDelimiter);
          if (title) {
            structureRanges.push(title);
            foundTitle = true;
          } else if (firstBracket && getCache().commandArgumentSpecs.get(command[1])?.[0]?.kind === 'keywords') {
            structureRanges.push({ start: bodyStart, end: close });
            foundTitle = true;
          }
          firstBracket = false;
        } else if (!foundTitle) {
          structureRanges.push({ start: cursor + 1, end: close });
          foundTitle = true;
        }
        cursor = close + 1;
      }
    }
    structureRanges.sort((left, right) => left.start - right.start);

    const styled = [];
    const mapped = new Map(rules.commandStyles);
    for (const [name, style] of rules.highlights) {
      mapped.set(name, mergeStyles(mapped.get(name), style));
    }
    function effective(style, index) {
      return structureRanges.some(range => index >= range.start && index < range.end)
        ? mergeStyles(style, { bold: true })
        : style;
    }
    function emit(start, end, style) {
      let cursor = start;
      while (cursor < end) {
        const nextBoundary = structureRanges.reduce((next, range) => {
          if (range.start > cursor && range.start < next) return range.start;
          if (range.end > cursor && range.end < next) return range.end;
          return next;
        }, end);
        addRange(styled, cursor, nextBoundary, effective(style, cursor));
        cursor = nextBoundary;
      }
    }
    function groupAfter(commandEnd, limit, allowOptional = true) {
      let cursor = commandEnd;
      while (/\s/.test(syntax[cursor] || '')) cursor += 1;
      if (allowOptional && syntax[cursor] === '[') {
        const optionEnd = findMatchingDelimiter(syntax, cursor, '[', ']');
        cursor = optionEnd + 1;
        while (/\s/.test(syntax[cursor] || '')) cursor += 1;
      }
      if (syntax[cursor] !== '{') return undefined;
      const close = findMatchingDelimiter(syntax, cursor, '{', '}');
      return close < limit ? { start: cursor, end: close, contentStart: cursor + 1, contentEnd: close } : undefined;
    }
    function scan(start, end, initialStyle = {}, scoped = false) {
      let style = initialStyle;
      let cursor = start;
      let plainStart = start;
      const flush = at => { emit(plainStart, at, style); };
      while (cursor < end) {
        if (text[cursor] === '%' && !isEscaped(text, cursor)) {
          flush(cursor);
          while (cursor < end && text[cursor] !== '\n' && text[cursor] !== '\r') cursor += 1;
          plainStart = cursor;
          continue;
        }
        if (syntax[cursor] === '\\') {
          flush(cursor);
          let commandEnd = cursor + 1;
          while (/[A-Za-z@]/.test(syntax[commandEnd] || '')) commandEnd += 1;
          const name = commandEnd > cursor + 1 ? syntax.slice(cursor + 1, commandEnd) : '';
          if (!name) commandEnd += 1;
          const commandStyle = mapped.get(name) || inferCommandStyle(name);
          if (name.toLowerCase() === 'color' || rules.colorCommands.has(name)) {
            let option = commandEnd;
            while (/\s/.test(syntax[option] || '')) option += 1;
            if (syntax[option] === '[') {
              const optionEnd = findMatchingDelimiter(syntax, option, '[', ']');
              const colorName = text.slice(option + 1, optionEnd).trim();
              const hex = rules.resolveColor(colorName);
              const wrapper = groupAfter(optionEnd + 1, end, false);
              if (wrapper) {
                scan(wrapper.contentStart, wrapper.contentEnd, mergeStyles(style, hex ? { color: `#${hex}` } : {}), true);
                cursor = wrapper.end + 1;
                plainStart = cursor;
                continue;
              }
              if (scoped) style = mergeStyles(style, hex ? { color: `#${hex}` } : {});
              cursor = optionEnd + 1;
              plainStart = cursor;
              continue;
            }
          } else if (commandStyle) {
            const wrapper = groupAfter(commandEnd, end);
            if (wrapper) {
              scan(wrapper.contentStart, wrapper.contentEnd, mergeStyles(style, commandStyle), true);
              cursor = wrapper.end + 1;
              plainStart = cursor;
              continue;
            }
            if (scoped) style = mergeStyles(style, commandStyle);
          }
          cursor = commandEnd;
          plainStart = cursor;
          continue;
        }
        if (syntax[cursor] === '{') {
          flush(cursor);
          const close = findMatchingDelimiter(syntax, cursor, '{', '}');
          if (close < end) {
            scan(cursor + 1, close, style, true);
            cursor = close + 1;
            plainStart = cursor;
            continue;
          }
        }
        cursor += 1;
      }
      flush(end);
    }
    scan(0, syntax.length);
    return styled;
  }

  function isEscaped(text, index) {
    let slashes = 0;
    while (index > 0 && text[--index] === '\\') slashes += 1;
    return slashes % 2 === 1;
  }

  function findTitleAssignment(body, bodyStart, matchDelimiter) {
    let index = 0;
    while (index < body.length) {
      while (/[\s,]/.test(body[index] || '')) index += 1;
      const key = body.slice(index).match(/^([\w-]+)\s*=/);
      if (!key) {
        while (index < body.length && body[index] !== ',') index += 1;
        continue;
      }
      const valueStart = index + key[0].length;
      let valueEnd = valueStart;
      while (valueEnd < body.length && body[valueEnd] !== ',') {
        if (body[valueEnd] === '{' || body[valueEnd] === '[') {
          const close = matchDelimiter(body, valueEnd, body[valueEnd], body[valueEnd] === '{' ? '}' : ']');
          valueEnd = close + 1;
        } else valueEnd += 1;
      }
      if (key[1].toLowerCase() === 'title') {
        while (/\s/.test(body[valueEnd - 1] || '')) valueEnd -= 1;
        let start = valueStart;
        while (/\s/.test(body[start] || '')) start += 1;
        if (body[start] === '{' && matchDelimiter(body, start, '{', '}') === valueEnd - 1) {
          return { start: bodyStart + start + 1, end: bodyStart + valueEnd - 1 };
        }
        return { start: bodyStart + start, end: bodyStart + valueEnd };
      }
      index = valueEnd + 1;
    }
    return undefined;
  }

  function decorationFor(style) {
    const key = JSON.stringify(style);
    if (!decorations.has(key)) {
      decorations.set(key, vscode.window.createTextEditorDecorationType({
        ...(style.bold && { fontWeight: 'bold' }),
        ...(style.italic && { fontStyle: 'italic' }),
        ...(style.color && { color: style.color })
      }));
    }
    return decorations.get(key);
  }

  function update(editor) {
    if (!editor) return;
    const groups = new Map();
    if (rules && isContextTexDocument(editor.document)) {
      for (const span of parseDocument(editor.document)) {
        const type = decorationFor(span.style);
        if (!groups.has(type)) groups.set(type, []);
        groups.get(type).push(new vscode.Range(editor.document.positionAt(span.start), editor.document.positionAt(span.end)));
      }
    }
    for (const type of decorations.values()) editor.setDecorations(type, groups.get(type) || []);
  }

  function updateVisibleEditors() {
    for (const editor of vscode.window.visibleTextEditors) update(editor);
  }

  async function refresh() {
    rules = await loadRules();
    updateVisibleEditors();
  }

  function dispose() {
    for (const decoration of decorations.values()) decoration.dispose();
    decorations.clear();
  }

  return { refresh, update, updateVisibleEditors, dispose };
}

module.exports = { createWysiwymDecorations, extractSetting, inferCommandStyle, mergeStyles };
