const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const zlib = require('zlib');
const vscode = require('vscode');

let cache = {
  xmlPath: '',
  commandMap: new Map(),
  commandCompletions: [],
  commandArgumentSpecs: new Map(),
  commandArgumentSpecVariants: new Map(),
  parameterValueDefaults: new Map()
};

// Keep track of PDFs opened by this extension. This also covers a PDF editor
// detached into another VS Code window, which is not visible in this window's
// tab groups.
const openedPdfPaths = new Set();
const ACADEMIC_PDF_VIEWER_EXTENSION_ID = 'ovolab-veritas.academic-pdf-viewer';
const ACADEMIC_PDF_VIEW_TYPE = 'academicPdfViewer.pdf';
const ACADEMIC_PDF_RELOAD_COMMAND = 'academicPdfViewer.reload';

function uniqueValues(values) {
  return Array.from(new Set(values.filter(Boolean)));
}

function normalizeTypeLabel(value) {
  const raw = String(value || '').trim();
  if (!raw) {
    return '';
  }

  const normalized = raw.startsWith('cd:') ? raw.slice(3) : raw;
  return normalized.toUpperCase();
}

function escapeHtmlText(value) {
  return String(value || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function buildHtmlKeyValueTable(headers, rows) {
  if (!rows || rows.length === 0) {
    return '';
  }

  const [headerKey, headerValues] = headers;
  const keyColumnWidthCh = Math.max(
    String(headerKey || '').length,
    ...rows.map((row) => String(row[0] || '').length)
  );
  const header = [
    '<table style="border-collapse:collapse; table-layout:auto; width:auto;">',
    '<thead>',
    '<tr>',
    `<th align="left" valign="top" style="text-align:left; vertical-align:top; white-space:nowrap; width:${keyColumnWidthCh}ch; min-width:${keyColumnWidthCh}ch; max-width:${keyColumnWidthCh}ch; padding:0 12px 2px 0;"><nobr>${escapeHtmlText(headerKey)}</nobr></th>`,
    `<th align="left" valign="top" style="text-align:left; vertical-align:top; padding:0 0 2px 0;">${escapeHtmlText(headerValues)}</th>`,
    '</tr>',
    '</thead>',
    '<tbody>'
  ].join('');

  const body = rows.map((row) => {
    const key = escapeHtmlText(String(row[0] || ''));
    const value = row[1] || '';
    return [
      '<tr>',
      `<td align="left" valign="top" style="text-align:left; vertical-align:top; white-space:nowrap; word-break:keep-all; overflow-wrap:normal; width:${keyColumnWidthCh}ch; min-width:${keyColumnWidthCh}ch; max-width:${keyColumnWidthCh}ch; padding:0 12px 0 0;"><nobr>${key}</nobr></td>`,
      `<td align="left" valign="top" style="text-align:left; vertical-align:top; padding:0;">${value}</td>`,
      '</tr>'
    ].join('');
  }).join('');

  return `${header}${body}</tbody></table>`;
}

function formatValueCell(value, isDefault = false) {
  const text = escapeHtmlText(value);
  return isDefault ? `<u>${text}</u>` : text;
}

function chunkValuesForDisplay(values, chunkSize = 8) {
  if (!Array.isArray(values) || values.length === 0) {
    return '';
  }

  const chunks = [];
  for (let i = 0; i < values.length; i += chunkSize) {
    chunks.push(values.slice(i, i + chunkSize).join(' '));
  }
  return chunks.join('<br/>');
}

function collectOrderedValues(values, defaults = new Set()) {
  const ordered = [];
  const seen = new Set();

  for (const value of values || []) {
    const key = String(value);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    ordered.push({ value: key, isDefault: defaults.has(key) });
  }

  return ordered;
}

function getDelimiterInfo(delimiter) {
  switch (delimiter) {
    case 'brace':
      return { open: '{', close: '}' };
    case 'paren':
      return { open: '(', close: ')' };
    case 'none':
      return { open: ' ', close: '' };
    default:
      return { open: '[', close: ']' };
  }
}

function describeArgumentSpec(spec) {
  const keywordTypes = uniqueValues(spec.keywordTypes || []).map(normalizeTypeLabel);
  const parameterNames = uniqueValues(spec.parameterNames || []);

  let core = '...';

  if (spec.kind === 'assignments') {
    core = spec.list ? '...=..., ...=...' : '...=...';
  } else if (spec.kind === 'keywords') {
    core = spec.list ? '..., ...' : '...';
  } else if (spec.kind === 'csname') {
    core = '...';
  } else if (spec.kind === 'content') {
    core = 'CONTENT';
  }

  return core;
}

function buildArgumentSnippet(argumentSpecs, startIndex = 1) {
  const parts = [];
  let tabIndex = startIndex;

  for (const spec of argumentSpecs || []) {
    if (spec.kind === 'content') {
      continue;
    }

    const delimiter = getDelimiterInfo(spec.delimiter);
    parts.push(`${delimiter.open}${'${' + tabIndex + '}'}${delimiter.close}`);
    tabIndex += 1;
  }

  return {
    text: parts.join(''),
    nextIndex: tabIndex
  };
}

function buildSignatureParts(commandName, argumentSpecs, options = {}) {
  const parts = [`\\${commandName}`];
  const parameters = [];
  const entry = getCommandEntry(commandName);
  const activeParameterIndex = Number.isInteger(options.activeParameterIndex) ? options.activeParameterIndex : -1;
  const activeAssignmentKey = options.activeAssignmentKey || '';

  for (let specIndex = 0; specIndex < (argumentSpecs || []).length; specIndex++) {
    const spec = argumentSpecs[specIndex];
    if (spec.kind === 'content') {
      continue;
    }

    const delimiter = getDelimiterInfo(spec.delimiter);
    const schema = describeArgumentSpec(spec);
    const argumentBody = `${delimiter.open}${schema}${delimiter.close}`;
    const text = argumentBody;
    const start = parts.join('').length;
    parts.push(text);
    const end = parts.join('').length;

    const docLines = [];

    if (spec.kind === 'assignments') {
      const parameterRows = [];
      const parameterNames = getArgumentParameterNames(spec, entry);
      for (const parameterName of parameterNames) {
        if (activeAssignmentKey && specIndex === activeParameterIndex && parameterName !== activeAssignmentKey) {
          continue;
        }

        const values = entry && entry.parameterValues.has(parameterName)
          ? collectOrderedValues(entry.parameterValues.get(parameterName), entry.parameterValueDefaults.get(parameterName))
          : [];
        const parameterTypes = entry && entry.parameterTypes && entry.parameterTypes.has(parameterName)
          ? uniqueValues(Array.from(entry.parameterTypes.get(parameterName)).map(normalizeTypeLabel))
          : [];

        const renderedValues = values.length > 0
          ? chunkValuesForDisplay(values.map(item => formatValueCell(item.value, item.isDefault)))
          : chunkValuesForDisplay(parameterTypes);

        parameterRows.push([parameterName, renderedValues || '']);
      }
      if (spec.allowsArbitraryKeys) {
        parameterRows.push(['KEY', 'VALUE']);
      }

      const table = buildHtmlKeyValueTable(['Key', 'Values'], parameterRows);
      if (table) {
        docLines.push(table);
      }
    } else if (spec.kind === 'keywords') {
      const values = uniqueValues(spec.keywordValues || []);
      const types = uniqueValues((spec.keywordTypes || []).map(normalizeTypeLabel));

      if (values.length > 0) {
        docLines.push(values.map(value => formatValueCell(value)).join(' '));
      } else if (types.length > 0) {
        docLines.push(types.join(' '));
      }
    }

    if (docLines.length === 0) {
      docLines.push(argumentBody);
    }

    const documentation = new vscode.MarkdownString(docLines.join('  \n'));
    documentation.supportHtml = true;

    parameters.push({
      label: [start, end],
      documentation
    });
  }

  return {
    label: parts.join(''),
    parameters
  };
}

function makeCommandInsertText(commandName, argumentSpecs) {
  const argSnippet = buildArgumentSnippet(argumentSpecs, 1);
  if (!argSnippet.text) {
    return commandName;
  }
  return new vscode.SnippetString(`${commandName}${argSnippet.text}`);
}

/** Builds the visible argument shape for a command completion variant. */
function makeArgumentPreview(commandName, argumentSpecs) {
  const argumentsText = (argumentSpecs || [])
    .filter(spec => spec.kind !== 'content')
    .map(spec => {
      const delimiter = getDelimiterInfo(spec.delimiter);
      return `${delimiter.open}${delimiter.close}`;
    })
    .join('');
  return `\\${commandName}${argumentsText}`;
}

function parseAttributes(tag) {
  const attrs = {};
  const re = /(\w+)\s*=\s*"([^"]*)"/g;
  let m;
  while ((m = re.exec(tag)) !== null) {
    attrs[m[1]] = m[2];
  }
  return attrs;
}

function ensureEntry(map, name) {
  if (!map.has(name)) {
    map.set(name, {
      keywords: new Set(),
      parameters: new Set(),
      parameterValues: new Map(),
      parameterTypes: new Map(),
        parameterValueDefaults: new Map(),
      inherits: new Set(),
      parameterSources: new Map()
    });
  }
  return map.get(name);
}

function getReferenceNames(xmlFragment) {
  const refs = [];
  const re = /<cd:(?:inherit|resolve)\b([^>]*)\/>/g;
  let m;
  while ((m = re.exec(xmlFragment)) !== null) {
    const attrs = parseAttributes(m[1]);
    const name = (attrs.name || '').trim();
    if (name) {
      refs.push(name);
    }
  }
  return refs;
}

function parseCommandMap(xmlText) {
  const commandMap = new Map();
  const completionCandidates = new Map();
  const commandArgumentSpecCandidates = new Map();

  function parseArgumentSpecs(commandBlock) {
    const out = [];
    const argumentsBlockMatch = commandBlock.match(/<cd:arguments\b[^>]*>([\s\S]*?)<\/cd:arguments>/);
    if (!argumentsBlockMatch) {
      return out;
    }

    const argumentsBlock = argumentsBlockMatch[1];
    const argBlockRe = /<cd:(keywords|assignments|content|csname)\b([^>]*?)(?:\/\>|>([\s\S]*?)<\/cd:\1>)/g;
    let am;
    while ((am = argBlockRe.exec(argumentsBlock)) !== null) {
      const kind = am[1];
      const attrs = parseAttributes(am[2]);
      const body = am[3] || '';
      const rawDelimiter = (attrs.delimiters || '').trim().toLowerCase();
      const delimiter = rawDelimiter === 'braces'
        ? 'brace'
        : rawDelimiter === 'parenthesis'
          ? 'paren'
          : rawDelimiter === 'none'
            ? 'none'
            : 'bracket';
      const optional = (attrs.optional || '').trim().toLowerCase() === 'yes';
      const list = (attrs.list || '').trim().toLowerCase() === 'yes';

      if (kind === 'keywords') {
        const keywordValues = [];
        const keywordTypes = [];
        const cRe = /<cd:constant\b([^>]*)\/>/g;
        let cm;
        while ((cm = cRe.exec(body)) !== null) {
          const cAttrs = parseAttributes(cm[1]);
          const explicitValue = (cAttrs.value || '').trim();
          const explicitType = (cAttrs.type || '').trim();

          if (explicitValue && !explicitValue.startsWith('cd:')) {
            keywordValues.push(explicitValue);
          }

          if (explicitType) {
            keywordTypes.push(explicitType);
          }
        }

        out.push({ kind, optional, delimiter, list, keywordValues, keywordTypes });
        continue;
      }

      if (kind === 'assignments') {
        const parameterNames = [];
        const parameterTypes = new Map();
        const parameterSources = getReferenceNames(body);
        let hasParameterTags = false;
        let allowsArbitraryKeys = false;
        const parameterRe = /<cd:parameter\b([^>]*?)(?:\/\>|>([\s\S]*?)<\/cd:parameter>)/g;
        let pm;
        while ((pm = parameterRe.exec(body)) !== null) {
          hasParameterTags = true;
          const pAttrs = parseAttributes(pm[1]);
          const parameterName = (pAttrs.name || '').trim();
          if (parameterName === 'cd:key') {
            allowsArbitraryKeys = true;
          }
          if (parameterName && parameterName !== 'cd:key') {
            parameterNames.push(parameterName);
            if (!parameterTypes.has(parameterName)) {
              parameterTypes.set(parameterName, []);
            }
          }

          const parameterBody = pm[2] || '';
          const cRe = /<cd:constant\b([^>]*)\/>/g;
          let cm;
          while ((cm = cRe.exec(parameterBody)) !== null) {
            const cAttrs = parseAttributes(cm[1]);
            const explicitType = (cAttrs.type || '').trim();
            if (explicitType && parameterName && parameterName !== 'cd:key') {
              parameterTypes.get(parameterName).push(explicitType);
            }
          }
        }

        out.push({ kind, optional, delimiter, list, parameterNames, parameterTypes, parameterSources, hasParameterTags, allowsArbitraryKeys });
        continue;
      }

      out.push({ kind, optional, delimiter, list });
    }

    return out;
  }

  function getRequiredBracketArgumentCount(commandBlock) {
    const argumentsBlockMatch = commandBlock.match(/<cd:arguments\b[^>]*>([\s\S]*?)<\/cd:arguments>/);
    if (!argumentsBlockMatch) {
      return 0;
    }

    const argumentsBlock = argumentsBlockMatch[1];
    const argStartRe = /<cd:(keywords|assignments)\b([^>]*)>/g;
    let count = 0;
    let match;

    while ((match = argStartRe.exec(argumentsBlock)) !== null) {
      const attrs = parseAttributes(match[2]);
      const optional = (attrs.optional || '').trim().toLowerCase();
      if (optional === 'yes') {
        continue;
      }

      const delimiter = (attrs.delimiters || '').trim().toLowerCase();
      if (delimiter === 'none' || delimiter === 'braces' || delimiter === 'parenthesis') {
        continue;
      }

      // ConTeXt defaults to square brackets when no explicit delimiter is given.
      count++;
    }

    return count;
  }

  function registerCompletion(label, item, meta = {}) {
    if (!completionCandidates.has(label)) {
      completionCandidates.set(label, []);
    }

    completionCandidates.get(label).push({
      kind: item.kind,
      insertText: item.insertText,
      sortWeight: item.sortWeight,
      meta
    });
  }

  function registerCommandArgumentSpecs(commandName, argumentSpecs, variant) {
    if (!commandName || !Array.isArray(argumentSpecs)) {
      return;
    }
    if (!commandArgumentSpecCandidates.has(commandName)) {
      commandArgumentSpecCandidates.set(commandName, []);
    }
    commandArgumentSpecCandidates.get(commandName).push({
      argumentSpecs,
      variant: (variant || '').toLowerCase()
    });
  }

  function getInsertTextValue(candidate) {
    if (!candidate || candidate.insertText === undefined || candidate.insertText === null) {
      return '';
    }

    if (typeof candidate.insertText === 'string') {
      return candidate.insertText;
    }

    if (typeof candidate.insertText.value === 'string') {
      return candidate.insertText.value;
    }

    return String(candidate.insertText);
  }

  function scoreCompletionCandidate(label, candidate) {
    const text = getInsertTextValue(candidate);
    const meta = candidate.meta || {};
    const normalizedVariant = String(meta.variant || '').toLowerCase();
    const requiredBracketCount = Number(meta.requiredBracketCount || 0);

    let score = 0;

    // Prefer canonical (non-string) XML variants.
    if (normalizedVariant === 'string') {
      score -= 200;
    } else if (normalizedVariant) {
      score += 20;
    } else {
      score += 140;
    }

    // Prefer environment-aware start/stop snippets where relevant.
    if (meta.type === 'environment') {
      score += 80;
      if (label.startsWith('\\start')) {
        score += 40;
      }
    }

    // Mandatory [] arguments are most important for this workflow.
    score += requiredBracketCount * 120;
    if (text.includes('[') && text.includes(']')) {
      score += 40;
    }
    if (/\$\{\d+/.test(text)) {
      score += 20;
    }

    if (typeof candidate.insertText !== 'string' && typeof candidate.insertText?.value === 'string') {
      score += 30;
    }

    // Stable tie-breaker.
    score += Math.min(text.length, 100) / 1000;
    return score;
  }

  function makeEnvironmentInsertText(startName, argumentSpecs) {
    const stopName = startName.startsWith('start') && startName.length > 5
      ? `stop${startName.slice(5)}`
      : `stop${startName}`;

    const argumentSnippet = buildArgumentSnippet(argumentSpecs, 1);
    return new vscode.SnippetString(`${startName}${argumentSnippet.text}\n\t$0\n\\${stopName}`);
  }

  /** Returns valid positional forms for the command's optional arguments. */
  function getArgumentSpecVariants(argumentSpecs) {
    if (argumentSpecs.length === 0) {
      return [[]];
    }

    const variants = [];
    const collect = (index, selected) => {
      if (index >= argumentSpecs.length) {
        if (selected.length > 0) {
          variants.push(selected);
        }
        return;
      }

      const spec = argumentSpecs[index];
      if (spec.optional) {
        collect(index + 1, [...selected, spec]);
        collect(index + 1, selected);
      } else {
        collect(index + 1, [...selected, spec]);
      }
    };
    collect(0, []);

    const validVariants = variants.filter(variant => {
      const last = variant[variant.length - 1];
      const lastIndex = argumentSpecs.indexOf(last);
      const hasSkippedAssignment = argumentSpecs
        .slice(lastIndex + 1)
        .some(spec => spec.optional && spec.kind === 'assignments');
      return !(last.optional && last.kind === 'keywords' && hasSkippedAssignment);
    });

    return [...new Map(validVariants.map(variant => {
      const shape = variant
        .map(spec => `${spec.kind}:${spec.delimiter}:${spec.list ? 'list' : 'single'}`)
        .join('|');
      return [shape, variant];
    })).values()];
  }

  const commandRe = /<cd:command\b[^>]*\/>|<cd:command\b[^>]*>[\s\S]*?<\/cd:command>/g;
  const commandBlocks = xmlText.match(commandRe) || [];

  for (const block of commandBlocks) {
    const openTag = block.match(/^<cd:command\b[^>]*\/?>/);
    if (!openTag) {
      continue;
    }

    const attrs = parseAttributes(openTag[0]);
    const name = attrs.name;
    const type = attrs.type || '';
    const variant = attrs.variant || '';
    const requiredBracketCount = getRequiredBracketArgumentCount(block);
    const argumentSpecs = parseArgumentSpecs(block);
    if (!name) {
      continue;
    }

    const entry = ensureEntry(commandMap, name);

    const instances = [];
    const instancesBlock = block.match(/<cd:instances\b[^>]*>[\s\S]*?<\/cd:instances>/);
    if (instancesBlock) {
      const iRe = /<cd:constant\b([^>]*)\/>/g;
      let im;
      while ((im = iRe.exec(instancesBlock[0])) !== null) {
        const iAttrs = parseAttributes(im[1]);
        const value = (iAttrs.value || '').trim();
        if (value) {
          instances.push(value);
        }
      }
    }

    if (type === 'environment') {
      if (instances.length > 0) {
        for (const instance of instances) {
          const startName = `start${instance}`;
          const label = `\\${startName}`;
          const item = {
            kind: vscode.CompletionItemKind.Function,
            insertText: makeEnvironmentInsertText(startName, argumentSpecs),
            sortWeight: '00'
          };

          commandMap.set(startName, entry);
          registerCompletion(label, item, {
            type,
            variant,
            requiredBracketCount,
            argumentSpecs,
            environmentStartName: startName
          });
          registerCommandArgumentSpecs(startName, argumentSpecs, variant);
        }
      } else {
        const startName = name.startsWith('start') ? name : `start${name}`;
        const label = `\\${startName}`;
        const item = {
          kind: vscode.CompletionItemKind.Function,
          insertText: makeEnvironmentInsertText(startName, argumentSpecs),
          sortWeight: '00'
        };

        commandMap.set(startName, entry);
        registerCompletion(label, item, {
          type,
          variant,
          requiredBracketCount,
          argumentSpecs,
          environmentStartName: startName
        });
        registerCommandArgumentSpecs(startName, argumentSpecs, variant);
      }
    } else {
      const insertText = makeCommandInsertText(name, argumentSpecs);

      registerCompletion(`\\${name}`, {
        kind: vscode.CompletionItemKind.Function,
        insertText,
        sortWeight: '10'
      }, {
        type,
        variant,
        requiredBracketCount,
        argumentSpecs,
        commandName: name
      });
      registerCommandArgumentSpecs(name, argumentSpecs, variant);

      for (const instance of instances) {
        const instanceInsertText = makeCommandInsertText(instance, argumentSpecs);

        registerCompletion(`\\${instance}`, {
          kind: vscode.CompletionItemKind.Function,
          insertText: instanceInsertText,
          sortWeight: '11'
        }, {
          type,
          variant,
          requiredBracketCount,
          argumentSpecs,
          commandName: instance
        });
        registerCommandArgumentSpecs(instance, argumentSpecs, variant);
      }
    }

    const keywordBlocks = block.match(/<cd:keywords\b[^>]*>[\s\S]*?<\/cd:keywords>/g) || [];
    for (const keywordBlock of keywordBlocks) {
      const cRe = /<cd:constant\b([^>]*)\/>/g;
      let cm;
      while ((cm = cRe.exec(keywordBlock)) !== null) {
        const cAttrs = parseAttributes(cm[1]);
        const value = (cAttrs.value || cAttrs.type || '').trim();
        if (!value || value.startsWith('cd:')) {
          continue;
        }
        entry.keywords.add(value);
        if ((cAttrs.default || '').trim().toLowerCase() === 'yes') {
          if (!entry.parameterValueDefaults.has(name)) {
            entry.parameterValueDefaults.set(name, new Set());
          }
          entry.parameterValueDefaults.get(name).add(value);
        }
      }

      for (const refName of getReferenceNames(keywordBlock)) {
        entry.inherits.add(refName);
      }
    }

    const assignmentBlocks = block.match(/<cd:assignments\b[^>]*>[\s\S]*?<\/cd:assignments>/g) || [];
    for (const assignmentBlock of assignmentBlocks) {
      const withoutParams = assignmentBlock.replace(/<cd:parameter\b[^>]*>[\s\S]*?<\/cd:parameter>/g, '');
      for (const refName of getReferenceNames(withoutParams)) {
        entry.inherits.add(refName);
      }
    }

    const pRe = /<cd:parameter\b([^>]*)>/g;
    let pm;
    while ((pm = pRe.exec(block)) !== null) {
      const pAttrs = parseAttributes(pm[1]);
      const pName = (pAttrs.name || '').trim();
      if (!pName || pName === 'cd:key') {
        continue;
      }

      entry.parameters.add(pName);
      if (!entry.parameterValues.has(pName)) {
        entry.parameterValues.set(pName, new Set());
      }
      if (!entry.parameterTypes.has(pName)) {
        entry.parameterTypes.set(pName, new Set());
      }
      if (!entry.parameterValueDefaults.has(pName)) {
        entry.parameterValueDefaults.set(pName, new Set());
      }

      const bodyStart = pm.index + pm[0].length;
      const bodyEnd = block.indexOf('</cd:parameter>', bodyStart);
      if (bodyEnd <= bodyStart) {
        continue;
      }

      const parameterBody = block.slice(bodyStart, bodyEnd);
      const pcRe = /<cd:constant\b([^>]*)\/>/g;
      let pcm;
      while ((pcm = pcRe.exec(parameterBody)) !== null) {
        const cAttrs = parseAttributes(pcm[1]);
        const value = (cAttrs.value || cAttrs.type || '').trim();
        const explicitType = (cAttrs.type || '').trim();
        if (explicitType) {
          entry.parameterTypes.get(pName).add(explicitType);
        }
        if (!value || value.startsWith('cd:')) {
          continue;
        }
        entry.parameterValues.get(pName).add(value);
        if ((cAttrs.default || '').trim().toLowerCase() === 'yes') {
          entry.parameterValueDefaults.get(pName).add(value);
        }
      }

      if (!entry.parameterSources.has(pName)) {
        entry.parameterSources.set(pName, new Set());
      }
      for (const refName of getReferenceNames(parameterBody)) {
        entry.parameterSources.get(pName).add(refName);
      }
    }
  }

  const resolved = new Map();

  function mergeResolvedInto(target, source) {
    for (const keyword of source.keywords) {
      target.keywords.add(keyword);
    }

    for (const parameter of source.parameters) {
      target.parameters.add(parameter);
    }

    for (const [parameter, values] of source.parameterValues.entries()) {
      if (!target.parameterValues.has(parameter)) {
        target.parameterValues.set(parameter, new Set());
      }
      const targetValues = target.parameterValues.get(parameter);
      for (const value of values) {
        targetValues.add(value);
      }
    }

    for (const [parameter, types] of source.parameterTypes.entries()) {
      if (!target.parameterTypes.has(parameter)) {
        target.parameterTypes.set(parameter, new Set());
      }
      const targetTypes = target.parameterTypes.get(parameter);
      for (const value of types) {
        targetTypes.add(value);
      }
    }

    for (const [parameter, defaults] of source.parameterValueDefaults.entries()) {
      if (!target.parameterValueDefaults.has(parameter)) {
        target.parameterValueDefaults.set(parameter, new Set());
      }
      const targetDefaults = target.parameterValueDefaults.get(parameter);
      for (const value of defaults) {
        targetDefaults.add(value);
      }
    }
  }

  function resolveEntry(name, stack = new Set()) {
    if (resolved.has(name)) {
      return resolved.get(name);
    }

    if (stack.has(name) || !commandMap.has(name)) {
      return { keywords: new Set(), parameters: new Set(), parameterValues: new Map(), parameterTypes: new Map(), parameterValueDefaults: new Map() };
    }

    stack.add(name);
    const raw = commandMap.get(name);
    const out = {
      keywords: new Set(raw.keywords),
      parameters: new Set(raw.parameters),
      parameterValues: new Map(),
      parameterTypes: new Map(),
      parameterValueDefaults: new Map()
    };

    for (const [parameter, values] of raw.parameterValues.entries()) {
      out.parameterValues.set(parameter, new Set(values));
    }

    for (const [parameter, types] of raw.parameterTypes.entries()) {
      out.parameterTypes.set(parameter, new Set(types));
    }

    for (const [parameter, defaults] of raw.parameterValueDefaults.entries()) {
      out.parameterValueDefaults.set(parameter, new Set(defaults));
    }

    for (const inheritedName of raw.inherits) {
      mergeResolvedInto(out, resolveEntry(inheritedName, stack));
    }

    for (const [parameter, sources] of raw.parameterSources.entries()) {
      if (!out.parameterValues.has(parameter)) {
        out.parameterValues.set(parameter, new Set());
      }
      const valueSet = out.parameterValues.get(parameter);
      for (const sourceName of sources) {
        const sourceEntry = resolveEntry(sourceName, stack);
        for (const keyword of sourceEntry.keywords) {
          valueSet.add(keyword);
        }
      }
    }

    for (const [parameter, defaults] of raw.parameterValueDefaults.entries()) {
      if (!out.parameterValueDefaults.has(parameter)) {
        out.parameterValueDefaults.set(parameter, new Set());
      }
      const valueSet = out.parameterValueDefaults.get(parameter);
      for (const value of defaults) {
        valueSet.add(value);
      }
    }

    stack.delete(name);
    resolved.set(name, out);
    return out;
  }

  for (const key of commandMap.keys()) {
    commandMap.set(key, resolveEntry(key));
  }

  const commandCompletions = [];
  const labelsWithArguments = new Set(
    [...completionCandidates.entries()]
      .filter(([, candidates]) => candidates.some(candidate => candidate.meta.argumentSpecs?.length > 0))
      .map(([label]) => label)
  );
  for (const [label, candidates] of completionCandidates.entries()) {
    if (!candidates || candidates.length === 0) {
      continue;
    }
    const variants = new Map();
    for (const candidate of candidates) {
      const argumentVariants = candidate.meta && candidate.meta.argumentSpecs
        ? getArgumentSpecVariants(candidate.meta.argumentSpecs)
        : [null];
      for (const argumentSpecs of argumentVariants) {
        if (argumentSpecs.length === 0 && candidate.meta.argumentSpecs.length > 0) {
          continue;
        }
        const meta = argumentSpecs
          ? { ...candidate.meta, argumentSpecs }
          : candidate.meta;
        const commandName = meta.commandName || label.slice(1);
        const insertText = argumentSpecs
          ? (meta.type === 'environment'
            ? makeEnvironmentInsertText(commandName, argumentSpecs)
            : makeCommandInsertText(commandName, argumentSpecs))
          : candidate.insertText;
        const expanded = { ...candidate, insertText, meta };
        const key = getInsertTextValue(expanded);
        const current = variants.get(key);
        if (!current || scoreCompletionCandidate(label, expanded) > scoreCompletionCandidate(label, current)) {
          variants.set(key, expanded);
        }
      }
    }
    let variantIndex = 0;
    for (const variant of variants.values()) {
      commandCompletions.push({
        label,
        kind: variant.kind,
        insertText: variant.insertText,
        sortWeight: variant.sortWeight,
        argumentSpecs: variant.meta.argumentSpecs || [],
        variantIndex: variantIndex++
      });
    }
  }

  const commandArgumentSpecs = new Map();
  const commandArgumentSpecVariants = new Map();
  for (const [commandName, candidates] of commandArgumentSpecCandidates.entries()) {
    if (!candidates || candidates.length === 0) {
      continue;
    }

    const variants = [...new Map(candidates
      .flatMap(candidate => getArgumentSpecVariants(candidate.argumentSpecs || []))
      .map(argumentSpecs => [JSON.stringify(argumentSpecs), argumentSpecs])).values()];
    commandArgumentSpecVariants.set(commandName, variants);

    let best = candidates[0];
    for (let i = 1; i < candidates.length; i++) {
      const current = candidates[i];
      const currentIsString = current.variant === 'string';
      const bestIsString = best.variant === 'string';
      if (bestIsString && !currentIsString) {
        best = current;
        continue;
      }
      if (!best.variant && current.variant) {
        continue;
      }
      if (best.variant && !current.variant) {
        best = current;
        continue;
      }
      if (current.argumentSpecs.length > best.argumentSpecs.length) {
        best = current;
      }
    }

    commandArgumentSpecs.set(commandName, best.argumentSpecs);
  }

  return {
    commandMap,
    commandCompletions: commandCompletions.filter(completion =>
      completion.argumentSpecs.length > 0 || !labelsWithArguments.has(completion.label)
    ),
    commandArgumentSpecs,
    commandArgumentSpecVariants
  };
}

function cleanFontToken(token) {
  return token
    .trim()
    .replace(/^\\s!/, '')
    .replace(/^\\/, '')
    .replace(/^\s+|\s+$/g, '');
}

function parseFontSourceFile(filePath) {
  const completions = [];
  if (!fs.existsSync(filePath)) {
    return completions;
  }

  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split(/\r?\n/);

  const styleNames = new Set();
  const alternativeNames = new Set();

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith('%')) {
      continue;
    }

    let match = line.match(/\\definefontstyle\s*\[([^\]]+)\]/);
    if (match) {
      for (const token of match[1].split(',')) {
        const name = cleanFontToken(token);
        if (name) {
          styleNames.add(name);
        }
      }
      continue;
    }

    match = line.match(/\\definefontalternative\s*\[([^\]]+)\]/);
    if (match) {
      for (const token of match[1].split(',')) {
        const name = cleanFontToken(token);
        if (name) {
          alternativeNames.add(name);
        }
      }
    }
  }

  const all = new Set();
  const sizeSuffixes = ['x', 'xx', 'a', 'b', 'c', 'd'];
  const expandable = new Set(['tf', 'bf', 'it', 'sl', 'bi', 'bs', 'sc']);

  for (const style of styleNames) {
    all.add(style);
  }

  for (const alt of alternativeNames) {
    all.add(alt);
    if (expandable.has(alt)) {
      for (const suffix of sizeSuffixes) {
        all.add(`${alt}${suffix}`);
      }
    }
  }

  all.add('tx');
  all.add('txx');

  for (const command of all) {
    completions.push({
      label: `\\${command}`,
      kind: vscode.CompletionItemKind.Function,
      insertText: command,
      sortWeight: '05'
    });
  }

  return completions;
}

function loadFontCompletions(xmlPath) {
  const texContextRoot = path.dirname(path.dirname(path.dirname(xmlPath)));
  const candidates = [
    path.join(texContextRoot, 'base', 'mkiv', 'font-ini.mkvi'),
    path.join(texContextRoot, 'base', 'mkiv', 'font-pre.mkiv'),
    path.join(texContextRoot, 'base', 'mkxl', 'font-ini.mklx'),
    path.join(texContextRoot, 'base', 'mkxl', 'font-pre.mkxl')
  ];

  const seen = new Set();
  const completions = [];
  for (const candidate of candidates) {
    for (const item of parseFontSourceFile(candidate)) {
      if (seen.has(item.label)) {
        continue;
      }
      seen.add(item.label);
      completions.push(item);
    }
  }

  return completions;
}

/** Loads an interface tree while preserving commented definitions. */
function loadInterfaceXml(xmlPath, visited = new Set()) {
  const absolutePath = path.resolve(xmlPath);
  if (visited.has(absolutePath) || !fs.existsSync(absolutePath)) {
    return '';
  }

  visited.add(absolutePath);
  const text = fs.readFileSync(absolutePath, 'utf8');
  const directory = path.dirname(absolutePath);
  const references = [...text.matchAll(/<cd:interfacefile\b[^>]*\bfilename="([^"]+)"[^>]*\/?>/g)];
  return text + references
    .map(([, filename]) => loadInterfaceXml(path.join(directory, filename), visited))
    .join('\n');
}

/** Expands interface definitions referenced by i-context.xml. */
function expandInterfaceDefinitions(xmlText) {
  const definitions = new Map();
  const defineRe = /<cd:define\b([^>]*)>([\s\S]*?)<\/cd:define>/g;
  let match;
  while ((match = defineRe.exec(xmlText)) !== null) {
    const name = parseAttributes(match[1]).name;
    if (name) {
      definitions.set(name, match[2]);
    }
  }

  function expand(fragment, stack = new Set()) {
    return fragment.replace(/<cd:resolve\b([^>]*?)\s*\/?>/g, (reference, attributes) => {
      const name = parseAttributes(attributes).name;
      const definition = definitions.get(name);
      if (!definition || stack.has(name)) {
        return reference;
      }
      const nextStack = new Set(stack);
      nextStack.add(name);
      return expand(definition, nextStack);
    });
  }

  return expand(xmlText);
}

function getCommandEntry(commandName) {
  if (cache.commandMap.has(commandName)) {
    return cache.commandMap.get(commandName);
  }

  if (commandName.startsWith('start') && commandName.length > 5) {
    const baseName = commandName.slice(5);
    if (cache.commandMap.has(baseName)) {
      return cache.commandMap.get(baseName);
    }

    const setupName = `setup${baseName}`;
    if (cache.commandMap.has(setupName)) {
      return cache.commandMap.get(setupName);
    }
  }

  return null;
}

function resolveXmlPath(configuredXmlPath) {
  const configured = String(configuredXmlPath || '').trim();
  return configured && fs.existsSync(configured) ? configured : '';
}

function loadData(configuredTexRootPath, configuredXmlPath) {
  const texRootPath = resolveTexRootPath(configuredTexRootPath, configuredXmlPath);
  const xmlPath = resolveXmlPathFromTexRoot(texRootPath) || resolveXmlPath(configuredXmlPath);
  if (!xmlPath) {
    cache = { xmlPath: '', commandMap: new Map(), commandCompletions: [], commandArgumentSpecs: new Map(), commandArgumentSpecVariants: new Map(), parameterValueDefaults: new Map() };
    return;
  }

  if (cache.xmlPath === xmlPath && cache.commandCompletions.length > 0) {
    return;
  }

  let xmlText = '';
  try {
    xmlText = expandInterfaceDefinitions(loadInterfaceXml(xmlPath));
  } catch (err) {
      vscode.window.showWarningMessage(`ConTeXt IntelliSense: Could not read i-context.xml below ${xmlPath}. Set contextIntellisense.texRootPath in settings.`);
    cache = { xmlPath, commandMap: new Map(), commandCompletions: [], commandArgumentSpecs: new Map(), commandArgumentSpecVariants: new Map(), parameterValueDefaults: new Map() };
    return;
  }

  const parsed = parseCommandMap(xmlText);
  const fontCompletions = loadFontCompletions(xmlPath);
  cache = {
    xmlPath,
    commandMap: parsed.commandMap,
    commandCompletions: parsed.commandCompletions.concat(fontCompletions),
    commandArgumentSpecs: parsed.commandArgumentSpecs,
    commandArgumentSpecVariants: parsed.commandArgumentSpecVariants,
    parameterValueDefaults: new Map(Array.from(parsed.commandMap.entries()).map(([name, entry]) => [name, entry.parameterValueDefaults || new Map()]))
  };
}

/** Counts consecutive square-bracket arguments after a command. */
function countBracketArguments(text) {
  let count = 0;
  let position = 0;

  while (position < text.length) {
    while (/\s/.test(text[position] || '')) {
      position++;
    }
    if (text[position] !== '[') {
      break;
    }

    count++;
    let depth = 1;
    position++;
    while (position < text.length && depth > 0) {
      if (text[position] === '[') {
        depth++;
      } else if (text[position] === ']') {
        depth--;
      }
      position++;
    }
  }

  return count;
}

/** Returns the square-bracket argument under the cursor. */
function getBracketInvocationContext(linePrefix, lineSuffix = '') {
  const m = linePrefix.match(/\\([A-Za-z@:_!?]+)((?:\s*\[[^\]]*\])*)(\s*\[[^\]]*)$/);
  if (!m) {
    return null;
  }

  const commandName = m[1];
  const completedBrackets = m[2] || '';
  const openBracketPart = m[3] || '';
  const currentSegment = openBracketPart.replace(/^\s*\[/, '');
  const remainingSegment = (lineSuffix.match(/^[^\]]*/) || [''])[0];
  const completedCount = (completedBrackets.match(/\[/g) || []).length;
  const commandToken = `\\${commandName}`;
  const commandIndex = linePrefix.lastIndexOf(commandToken);
  const argumentText = linePrefix.slice(commandIndex + commandToken.length) + lineSuffix;

  return {
    commandName,
    argumentIndex: completedCount,
    bracketCount: Math.max(completedCount + 1, countBracketArguments(argumentText)),
    currentSegment,
    remainingSegment
  };
}

/** Returns the current square-bracket argument context at a document position. */
function getDocumentBracketContext(document, position) {
  const line = document.lineAt(position.line).text;
  const prefix = line.slice(0, position.character);
  const suffix = line.slice(position.character);
  return getBracketInvocationContext(prefix, suffix);
}

/** Returns the current square-bracket argument context of an editor. */
function getEditorBracketContext(editor) {
  return getDocumentBracketContext(editor.document, editor.selection.active);
}

/** Returns the text of the current comma-separated argument segment. */
function getCurrentArgumentSegment(context) {
  const match = (context && context.currentSegment || '').match(/(?:^|,)\s*([^,\]]*)$/);
  return match ? match[1] : '';
}

/** Returns an empty argument at a document position. */
function getEmptyArgumentContext(document, position) {
  const context = getDocumentBracketContext(document, position);
  return context
    && !getCurrentArgumentSegment(context).trim()
    && !context.remainingSegment.trim()
    && getCommandEntry(context.commandName)
    ? context
    : null;
}

/** Opens completion suggestions when the cursor enters an empty argument. */
function triggerEmptyArgumentSuggestions(editor, refresh = false) {
  if (!editor || editor.document.languageId !== 'context.tex') {
    return;
  }

  const context = getEmptyArgumentContext(editor.document, editor.selection.active);
  const key = context
    ? `${editor.document.uri.toString()}:${editor.selection.active.line}:${editor.selection.active.character}`
    : '';
  if (!context || (!refresh && triggerEmptyArgumentSuggestions.lastPosition === key)) {
    if (!context) {
      triggerEmptyArgumentSuggestions.lastPosition = '';
    }
    return;
  }

  triggerEmptyArgumentSuggestions.lastPosition = key;
  if (!refresh) {
    void vscode.commands.executeCommand('editor.action.triggerSuggest');
    return;
  }

  void vscode.commands.executeCommand('hideSuggestWidget')
    .then(() => setTimeout(() => vscode.commands.executeCommand('editor.action.triggerSuggest'), 0));
}

/** Shows argument information whenever the cursor is inside an argument. */
function triggerArgumentInformation(editor, refresh = false, leftArgument = false) {
  if (!editor || editor.document.languageId !== 'context.tex') {
    return;
  }

  const context = getEditorBracketContext(editor);
  if (!context) {
    const line = editor.document.lineAt(editor.selection.active.line).text;
    const prefix = line.slice(0, editor.selection.active.character);
    if (!leftArgument && /\\[A-Za-z@:_!?]*$/.test(prefix)) {
      return;
    }
    void vscode.commands.executeCommand('hideSuggestWidget');
    void vscode.commands.executeCommand('closeParameterHints');
    return;
  }

  void vscode.commands.executeCommand('editor.action.triggerParameterHints');
  triggerEmptyArgumentSuggestions(editor, refresh);
}

/** Evaluates argument context after VS Code has applied the new selection. */
function scheduleArgumentInformation(editor, refresh = false, selectionChange = false) {
  const key = editor ? editor.document.uri.toString() : '';
  const hasArgument = !!(editor && getEditorBracketContext(editor));
  const leftArgument = selectionChange
    && scheduleArgumentInformation.lastArgumentState.get(key) === true
    && !hasArgument;
  scheduleArgumentInformation.lastArgumentState.set(key, hasArgument);
  setTimeout(() => triggerArgumentInformation(editor, refresh, leftArgument), 0);
}

scheduleArgumentInformation.lastArgumentState = new Map();

function takeBracketArguments(specs, count) {
  let seen = 0;
  const result = [];
  for (const spec of specs || []) {
    result.push(spec);
    if (spec.delimiter === 'bracket' && ++seen >= count) {
      break;
    }
  }
  return result;
}

function getCommandArgumentSpec(commandName, argumentIndex, bracketCount = argumentIndex + 1) {
  const specs = getCommandSignatureSpecs(commandName, bracketCount);
  if (!specs) {
    return null;
  }

  const bracketSpecs = specs.filter(spec => spec.delimiter === 'bracket');
  if (argumentIndex < 0 || argumentIndex >= bracketSpecs.length) {
    return null;
  }
  return bracketSpecs[argumentIndex];
}

/** Returns whether a signature still requires an argument after its brackets. */
function hasRequiredArgumentAfterBrackets(specs, bracketCount) {
  let seenBrackets = 0;
  for (const spec of specs || []) {
    if (spec.delimiter === 'bracket' && ++seenBrackets >= bracketCount) {
      return (specs || []).some((candidate, index) => index > specs.indexOf(spec) && !candidate.optional);
    }
  }
  return false;
}

/** Returns whether all entered bracket arguments are required by the XML. */
function hasRequiredBracketArguments(specs, bracketCount) {
  return (specs || [])
    .filter(spec => spec.delimiter === 'bracket')
    .slice(0, bracketCount)
    .every(spec => !spec.optional);
}

/** Returns the kind of the first bracket argument in a signature. */
function getFirstBracketKind(specs) {
  const first = (specs || []).find(spec => spec.delimiter === 'bracket');
  return first ? first.kind : '';
}

/** Merges metadata from equivalent XML argument definitions. */
function mergeArgumentSpec(left, right) {
  const parameterTypes = left.parameterTypes && left.parameterTypes.size > 0
    ? left.parameterTypes
    : right.parameterTypes || new Map();
  return {
    ...left,
    parameterNames: left.parameterNames && left.parameterNames.length > 0 ? left.parameterNames : right.parameterNames || [],
    parameterSources: left.parameterSources && left.parameterSources.length > 0 ? left.parameterSources : right.parameterSources || [],
    keywordValues: left.keywordValues && left.keywordValues.length > 0 ? left.keywordValues : right.keywordValues || [],
    keywordTypes: left.keywordTypes && left.keywordTypes.length > 0 ? left.keywordTypes : right.keywordTypes || [],
    parameterTypes,
    allowsArbitraryKeys: left.allowsArbitraryKeys || right.allowsArbitraryKeys,
    hasParameterTags: left.hasParameterTags || right.hasParameterTags
  };
}

/** Merges complete argument variants with the same XML shape. */
function mergeEquivalentArgumentVariants(variants) {
  const merged = new Map();
  for (const variant of variants) {
    const shape = variant
      .map(spec => `${spec.kind}:${spec.delimiter}:${spec.list ? 'list' : 'single'}`)
      .join('|');
    const existing = merged.get(shape);
    if (!existing) {
      merged.set(shape, variant.map(spec => ({ ...spec })));
      continue;
    }
    for (let index = 0; index < existing.length; index++) {
      existing[index] = mergeArgumentSpec(existing[index], variant[index]);
    }
  }
  return [...merged.values()];
}

function getCommandSignatureSpecs(commandName, bracketCount = 0) {
  if (bracketCount > 0) {
    const variants = cache.commandArgumentSpecVariants.get(commandName) || [];
    const matching = variants.filter(specs =>
      specs.filter(spec => spec.delimiter === 'bracket').length === bracketCount
    );
    for (const specs of variants) {
      if (specs.filter(spec => spec.delimiter === 'bracket').length > bracketCount) {
        matching.push(takeBracketArguments(specs, bracketCount));
      }
    }
    const mergedMatching = mergeEquivalentArgumentVariants(matching);
    const complete = mergedMatching.filter(specs => !hasRequiredArgumentAfterBrackets(specs, bracketCount));
    if (complete.length > 0) {
      const firstKind = getFirstBracketKind(complete[0]);
      const requiredDifferentKind = complete.find(specs =>
        hasRequiredBracketArguments(specs, bracketCount)
        && getFirstBracketKind(specs) !== firstKind
      );
      return requiredDifferentKind || complete[0];
    }
    if (mergedMatching.length > 0) {
      return mergedMatching[0];
    }
  }
  return cache.commandArgumentSpecs.get(commandName) || [];
}

/** Returns direct and inherited keys available in an assignment argument. */
function getArgumentParameterNames(argumentSpec, entry) {
  const names = [
    ...(argumentSpec ? argumentSpec.parameterNames || [] : [])
  ];

  for (const sourceName of argumentSpec && argumentSpec.parameterSources || []) {
    const source = getCommandEntry(sourceName);
    if (source) {
      names.push(...source.parameters);
    }
  }

  return uniqueValues(names);
}

const EXTERNAL_FIGURE_EXTENSIONS = new Set([
  'pdf', 'png', 'jpg', 'jpeg', 'jbig', 'jb2', 'jp2', 'jpx',
  'mps', 'svg', 'eps', 'gif', 'tif', 'tiff', 'webp', 'u3d', 'swf'
]);

/** Returns workspace files accepted by ConTeXt's externalfigure command. */
async function createExternalFigureItems() {
  const files = await vscode.workspace.findFiles(
    '**/*.{pdf,png,jpg,jpeg,jbig,jb2,jp2,jpx,mps,svg,eps,gif,tif,tiff,webp,u3d,swf}',
    '**/{.git,node_modules}/**'
  );
  return files
    .filter(uri => EXTERNAL_FIGURE_EXTENSIONS.has(path.extname(uri.fsPath).slice(1).toLowerCase()))
    .sort((left, right) => left.fsPath.localeCompare(right.fsPath))
    .map(uri => {
      const value = vscode.workspace.asRelativePath(uri, false).replace(/\\/g, '/');
      const item = new vscode.CompletionItem(value, vscode.CompletionItemKind.File);
      item.insertText = value;
      item.filterText = value;
      item.detail = 'Workspace file';
      return item;
    });
}

/** Extracts BibTeX entry keys from a source file. */
function parseBibtexKeys(content) {
  const keys = [];
  const entryPattern = /@([a-z]+)\s*\{\s*([^,\s]+)\s*,/gi;
  let match;
  while ((match = entryPattern.exec(content)) !== null) {
    if (!['string', 'preamble', 'comment'].includes(match[1].toLowerCase())) {
      keys.push(match[2]);
    }
  }
  return keys;
}

/** Returns BibTeX keys from every workspace .bib file. */
async function createCitationItems() {
  const files = await vscode.workspace.findFiles('**/*.bib', '**/{.git,node_modules}/**');
  const keys = new Set();
  for (const uri of files) {
    try {
      for (const key of parseBibtexKeys(await fs.promises.readFile(uri.fsPath, 'utf8'))) {
        keys.add(key);
      }
    } catch {
      // Ignore files that disappear or cannot be read while completion runs.
    }
  }
  return Array.from(keys).sort((left, right) => left.localeCompare(right)).map(key => {
    const item = new vscode.CompletionItem(key, vscode.CompletionItemKind.Reference);
    item.insertText = key;
    item.filterText = key;
    item.detail = 'BibTeX key';
    return item;
  });
}

/** Returns completion items for special workspace-backed bracket arguments. */
async function createWorkspaceArgumentItems(commandName) {
  if (commandName === 'externalfigure') {
    return createExternalFigureItems();
  }
  if (commandName === 'cite') {
    return createCitationItems();
  }
  return null;
}

/** Returns whether a workspace-backed completion belongs to this argument. */
function isWorkspaceArgument(commandName, argumentIndex, context) {
  if (commandName === 'externalfigure') {
    return argumentIndex === 0;
  }
  if (commandName === 'cite') {
    return context && argumentIndex === context.bracketCount - 1;
  }
  return false;
}

function getActiveParameterIndex(commandName, linePrefix) {
  const specs = getCommandSignatureSpecs(commandName).filter(spec => spec.kind !== 'content');
  if (specs.length === 0) {
    return 0;
  }

  const commandToken = `\\${commandName}`;
  const commandIndex = linePrefix.lastIndexOf(commandToken);
  if (commandIndex < 0) {
    return 0;
  }

  const suffix = linePrefix.slice(commandIndex + commandToken.length);
  let pos = 0;

  for (let i = 0; i < specs.length; i++) {
    const spec = specs[i];
    const delimiter = getDelimiterInfo(spec.delimiter);

    while (pos < suffix.length && /\s/.test(suffix[pos]) && spec.delimiter !== 'none') {
      pos += 1;
    }

    if (pos >= suffix.length) {
      return i;
    }

    if (spec.delimiter === 'none') {
      return i;
    }

    if (suffix[pos] !== delimiter.open) {
      return i;
    }

    pos += 1;
    let depth = 1;
    while (pos < suffix.length && depth > 0) {
      const char = suffix[pos];
      if (char === delimiter.open) {
        depth += 1;
      } else if (char === delimiter.close) {
        depth -= 1;
      }
      pos += 1;
    }

    if (depth > 0) {
      return i;
    }
  }

  return Math.max(specs.length - 1, 0);
}

function getActiveAssignmentKey(currentSegment) {
  const rawSegment = currentSegment || '';
  const segmentMatch = rawSegment.match(/(?:^|,)\,?\s*([^,\]]*)$/);
  const segment = segmentMatch ? segmentMatch[1] : rawSegment;
  const eqIndex = segment.indexOf('=');
  if (eqIndex < 0) {
    return '';
  }

  return segment.slice(0, eqIndex).trim();
}

/** Returns the stable order key for a command's argument form. */
function getCommandVariantSortKey(argumentSpecs) {
  const specs = (argumentSpecs || []).filter(spec => spec.kind !== 'content');
  if (specs.length === 0) {
    return '0_00';
  }

  const bracketCount = specs.filter(spec => spec.delimiter === 'bracket').length;
  if (bracketCount > 0) {
    return `1_${String(bracketCount).padStart(2, '0')}`;
  }
  if (specs.some(spec => spec.delimiter === 'brace')) {
    return '2_00';
  }
  return '3_00';
}

function createCommandItems(prefixPart) {
  const lower = (prefixPart || '').toLowerCase();
  const items = [];

  for (const completion of cache.commandCompletions) {
    const label = completion.label;
    const compareText = label.startsWith('\\') ? label.slice(1) : label;
    if (!compareText.toLowerCase().startsWith(lower)) {
      continue;
    }

    const argumentSpecs = completion.argumentSpecs || getCommandSignatureSpecs(compareText);
    const item = new vscode.CompletionItem(makeArgumentPreview(compareText, argumentSpecs), completion.kind);
    item.insertText = completion.insertText;
    item.sortText = `${completion.sortWeight}_${compareText}_${getCommandVariantSortKey(argumentSpecs)}_${completion.variantIndex || 0}`;
    item.filterText = label;
    const signatureParts = buildSignatureParts(compareText, argumentSpecs);
    item.detail = signatureParts.label || 'ConTeXt IntelliSense';
    if (argumentSpecs.length > 0) {
      item.command = {
        command: 'contextIntellisense.triggerArgumentContext',
        title: 'Trigger argument information'
      };
    }

    items.push(item);
  }

  return items;
}

function createKeywordAndParameterItems(commandName, currentSegment, argumentIndex = 0, bracketCount = argumentIndex + 1) {
  const entry = getCommandEntry(commandName);
  if (!entry) {
    return [];
  }

  const argumentSpec = getCommandArgumentSpec(commandName, argumentIndex, bracketCount);
  if (argumentSpec && argumentSpec.kind === 'keywords') {
    const segment = (currentSegment || '').trim();
    if (segment.startsWith('\\')) {
      return createCommandItems(segment.slice(1));
    }

    const values = argumentSpec.keywordValues || [];
    if (values.length === 0) {
      return [];
    }

    const out = [];
    for (const value of values) {
      if (segment && !value.toLowerCase().startsWith(segment.toLowerCase())) {
        continue;
      }
      const item = new vscode.CompletionItem(value, vscode.CompletionItemKind.Keyword);
      item.insertText = value;
      item.sortText = `0_${value}`;
      item.filterText = value;
      out.push(item);
    }
    return out;
  }

  if (argumentSpec && argumentSpec.kind !== 'assignments') {
    return [];
  }

  function createAssignmentItems(filterSegment = '') {
    const out = [];

    const parameterNames = getArgumentParameterNames(argumentSpec, entry);
    for (const parameter of parameterNames) {
      const label = `${parameter}=`;
      const item = new vscode.CompletionItem(label, vscode.CompletionItemKind.Field);
      item.insertText = new vscode.SnippetString(`${parameter}=$0`);
      item.command = {
        command: 'editor.action.triggerSuggest',
        title: 'Trigger suggest for parameter values'
      };
      item.sortText = `1_${label}`;
      item.filterText = label;
      out.push(item);
    }

    const normalizedFilter = (filterSegment || '').trim().toLowerCase();
    if (normalizedFilter) {
      return out.filter(item => item.label.toString().toLowerCase().startsWith(normalizedFilter));
    }

    return out;
  }

  const rawSegment = currentSegment || '';
  const segment = rawSegment.trim();
  if (rawSegment.includes('=')) {
    const eqIndex = rawSegment.indexOf('=');
    const key = rawSegment.slice(0, eqIndex).trim();
    const valuePartRaw = rawSegment.slice(eqIndex + 1);
    const valuePart = valuePartRaw.trim();

    if (/^\s*\\/.test(valuePartRaw)) {
      return createCommandItems(valuePartRaw.replace(/^\s*\\/, ''));
    }

    if (argumentSpec && argumentSpec.kind === 'assignments' && argumentSpec.allowsArbitraryKeys) {
      // Arbitrary assignments intentionally do not suggest constrained values.
      return [];
    }

    const values = entry.parameterValues.get(key);
    if (!values || values.size === 0) {
      // In value position with free-form values, do not show key suggestions.
      return [];
    }

    const out = [];
    for (const value of values) {
      if (valuePart && !value.toLowerCase().startsWith(valuePart.toLowerCase())) {
        continue;
      }
      const item = new vscode.CompletionItem(value, vscode.CompletionItemKind.Value);
      item.insertText = value;
      item.sortText = `0_${value}`;
      item.filterText = value;
      out.push(item);
    }

    return out;
  }

  if (argumentSpec && argumentSpec.kind === 'assignments') {
    return createAssignmentItems(segment);
  }

  const out = [];

  for (const keyword of entry.keywords) {
    const item = new vscode.CompletionItem(keyword, vscode.CompletionItemKind.Keyword);
    item.insertText = keyword;
    item.sortText = `0_${keyword}`;
    item.filterText = keyword;
    out.push(item);
  }

  for (const item of createAssignmentItems('')) {
    out.push(item);
  }

  if (segment) {
    return out.filter(item => item.label.toString().toLowerCase().startsWith(segment.toLowerCase()));
  }

  return out;
}

function isConTeXtTexFilePath(filePath) {
  const ext = path.extname(String(filePath || '')).toLowerCase();
  return ['.tex', '.mkiv', '.mkxl', '.mkvi', '.mkil', '.mkix', '.mkxi', '.mklx'].includes(ext);
}

async function applyContextLanguageToOpenDocuments() {
  const updates = [];
  for (const document of vscode.workspace.textDocuments) {
    if (!document || document.uri.scheme !== 'file') {
      continue;
    }

    if (!isConTeXtTexFilePath(document.uri.fsPath)) {
      continue;
    }

    if (document.languageId === 'context.tex') {
      continue;
    }

    updates.push(vscode.languages.setTextDocumentLanguage(document, 'context.tex'));
  }

  if (updates.length > 0) {
    await Promise.allSettled(updates);
  }
}

function isExistingDirectory(folderPath) {
  return !!folderPath && fs.existsSync(folderPath) && fs.statSync(folderPath).isDirectory();
}

function isExistingFile(filePath) {
  return !!filePath && fs.existsSync(filePath) && fs.statSync(filePath).isFile();
}

function resolveConfiguredPath(inputPath, workspaceFolderPath = '') {
  const value = String(inputPath || '').trim();
  if (!value) {
    return '';
  }

  if (path.isAbsolute(value)) {
    return value;
  }

  if (workspaceFolderPath) {
    return path.resolve(workspaceFolderPath, value);
  }

  return path.resolve(value);
}

function collectWorkspaceTemporaryFiles(workspaceFolders) {
  const disposableExtensions = new Set(['.log', '.tuc', '.pgf', '.synctex']);
  const files = new Set();

  for (const workspaceFolder of workspaceFolders || []) {
    if (!workspaceFolder || !workspaceFolder.uri || workspaceFolder.uri.scheme !== 'file') {
      continue;
    }

    const rootPath = path.resolve(workspaceFolder.uri.fsPath);
    if (!isExistingDirectory(rootPath)) {
      continue;
    }

    const pendingFolders = [rootPath];
    while (pendingFolders.length > 0) {
      const folderPath = pendingFolders.pop();
      let entries;
      try {
        entries = fs.readdirSync(folderPath, { withFileTypes: true });
      } catch (error) {
        continue;
      }

      const fileNames = new Set(
        entries.filter(entry => entry.isFile()).map(entry => entry.name.toLowerCase())
      );

      for (const entry of entries) {
        const fullPath = path.resolve(folderPath, entry.name);
        const relativePath = path.relative(rootPath, fullPath);
        if (relativePath === '..' || relativePath.startsWith(`..${path.sep}`) || path.isAbsolute(relativePath)) {
          continue;
        }

        // Deliberately do not follow directory symlinks or junctions.
        if (entry.isDirectory()) {
          pendingFolders.push(fullPath);
          continue;
        }
        if (!entry.isFile()) {
          continue;
        }

        const extension = path.extname(entry.name).toLowerCase();
        if (disposableExtensions.has(extension) || entry.name.toLowerCase().endsWith('.synctex.gz')) {
          files.add(fullPath);
          continue;
        }

        if (extension === '.pdf') {
          const texFileName = `${path.basename(entry.name, extension)}.tex`.toLowerCase();
          if (fileNames.has(texFileName)) {
            files.add(fullPath);
          }
        }
      }
    }
  }

  return Array.from(files).sort((a, b) => a.localeCompare(b));
}

function searchForFile(rootPath, targetNames, maxDepth = 6) {
  const normalizedTargets = new Set((targetNames || []).map((name) => String(name).toLowerCase()));
  if (!isExistingDirectory(rootPath)) {
    return '';
  }

  const queue = [{ folderPath: rootPath, depth: 0 }];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current || current.depth > maxDepth) {
      continue;
    }

    let entries = [];
    try {
      entries = fs.readdirSync(current.folderPath, { withFileTypes: true });
    } catch (error) {
      continue;
    }

    for (const entry of entries) {
      const fullPath = path.join(current.folderPath, entry.name);
      if (entry.isFile() && normalizedTargets.has(entry.name.toLowerCase())) {
        return fullPath;
      }
      if (entry.isDirectory()) {
        queue.push({ folderPath: fullPath, depth: current.depth + 1 });
      }
    }
  }

  return '';
}

function resolveTexRootPath(configuredTexRootPath, legacyXmlPath = '') {
  const configured = resolveConfiguredPath(configuredTexRootPath);
  if (isExistingDirectory(configured)) {
    return configured;
  }

  const legacy = resolveConfiguredPath(legacyXmlPath);
  if (isExistingFile(legacy)) {
    let current = path.dirname(legacy);
    for (let i = 0; i < 8 && current && current !== path.dirname(current); i++) {
      const discovered = resolveXmlPathFromTexRoot(current);
      if (discovered) {
        return current;
      }
      current = path.dirname(current);
    }
  }

  return configured;
}

function resolveXmlPathFromTexRoot(texRootPath) {
  const root = resolveConfiguredPath(texRootPath);
  if (!isExistingDirectory(root)) {
    return '';
  }

  const candidates = [
    root,
    path.join(root, 'tex'),
    path.join(root, 'tex', 'context'),
    path.join(root, 'tex', 'context', 'interface'),
    path.join(root, 'tex', 'context', 'interface', 'mkiv'),
    path.join(root, 'texmf-context', 'tex', 'context', 'interface', 'mkiv'),
    path.join(root, 'share', 'texmf-context', 'tex', 'context', 'interface', 'mkiv')
  ];

  for (const candidate of candidates) {
    const resolved = searchForFile(candidate, ['i-context.xml'], 6);
    if (resolved) {
      return resolved;
    }
  }

  return searchForFile(root, ['i-context.xml'], 7);
}

function resolveContextExecutableFromTexRoot(texRootPath) {
  const root = resolveConfiguredPath(texRootPath);
  if (!isExistingDirectory(root)) {
    return 'context';
  }

  const candidateFolders = [
    root,
    path.join(root, 'bin'),
    path.join(root, 'bin', 'windows'),
    path.join(root, 'bin', 'win32'),
    path.join(root, 'bin', 'x64'),
    path.join(root, 'scripts'),
    path.join(root, 'scripts', 'context'),
    path.join(root, 'scripts', 'context', 'lua'),
    path.join(root, 'tex'),
    path.join(root, 'tex', 'context'),
    path.join(root, 'tex', 'context', 'interface')
  ];

  const targetNames = process.platform === 'win32'
    ? ['context.exe', 'context.cmd', 'context.bat']
    : ['context', 'context.sh', 'context.lua'];

  for (const folderPath of candidateFolders) {
    const resolved = searchForFile(folderPath, targetNames, 5);
    if (resolved) {
      return resolved;
    }
  }

  return searchForFile(root, targetNames, 7) || 'context';
}

function resolveMainFilePath(configuredMainFilePath, workspaceFolderPath = '') {
  const resolved = resolveConfiguredPath(configuredMainFilePath, workspaceFolderPath);
  return isExistingFile(resolved) ? resolved : '';
}

function getAcademicPdfViewerExtension() {
  return vscode.extensions.getExtension(ACADEMIC_PDF_VIEWER_EXTENSION_ID);
}

/** Converts an in-workspace file path to a portable workspace-relative path. */
function toWorkspaceRelativePath(filePath, workspaceFolderPath) {
  const absoluteFilePath = path.resolve(filePath);
  const absoluteWorkspacePath = path.resolve(workspaceFolderPath);
  const relativePath = path.relative(absoluteWorkspacePath, absoluteFilePath);
  if (!relativePath || relativePath.startsWith(`..${path.sep}`) || path.isAbsolute(relativePath)) {
    return '';
  }
  return relativePath.split(path.sep).join('/');
}

/** Formats a filesystem path without exposing absolute workspace or external paths in traces. */
function formatSyncTexTracePath(filePath) {
  const value = String(filePath || '');
  if (!value) {
    return value;
  }
  const workspaceRoot = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0
    ? vscode.workspace.workspaceFolders[0].uri.fsPath
    : '';
  const relativePath = workspaceRoot ? toWorkspaceRelativePath(value, workspaceRoot) : '';
  return relativePath || '<external>';
}

/** Formats a file URI as a workspace-relative trace value. */
function formatSyncTexTraceUri(uriValue) {
  try {
    const uri = vscode.Uri.parse(String(uriValue || ''));
    return uri.scheme === 'file'
      ? `workspace:${formatSyncTexTracePath(uri.fsPath)}`
      : uri.toString();
  } catch {
    return '<invalid-uri>';
  }
}

function findOpenPdfGroup(pdfPath) {
  const normalizedPdfPath = path.normalize(pdfPath).toLowerCase();
  for (const group of vscode.window.tabGroups.all) {
    if (group.tabs.some((tab) => {
      const uri = tab.input && tab.input.uri;
      return uri && uri.scheme === 'file' && path.normalize(uri.fsPath).toLowerCase() === normalizedPdfPath;
    })) {
      return group;
    }
  }
  return null;
}

function forgetClosedPdfTabs(event) {
  for (const tab of event.closed || []) {
    const uri = tab.input && tab.input.uri;
    if (uri && uri.scheme === 'file' && path.extname(uri.fsPath).toLowerCase() === '.pdf') {
      openedPdfPaths.delete(path.normalize(uri.fsPath).toLowerCase());
    }
  }
}

/** Returns installed custom editors that can open PDF files. */
function getInstalledPdfViewers() {
  const viewers = [];
  for (const extension of vscode.extensions.all) {
    if (!extension || extension.id === ACADEMIC_PDF_VIEWER_EXTENSION_ID) {
      continue;
    }
    const customEditors = extension.packageJSON && extension.packageJSON.contributes
      ? extension.packageJSON.contributes.customEditors
      : [];
    for (const editor of customEditors || []) {
      const selectors = Array.isArray(editor.selector) ? editor.selector : [];
      const supportsPdf = selectors.some((selector) => {
        const pattern = String(selector && selector.filenamePattern || '').toLowerCase();
        return pattern === '*.pdf' || pattern.endsWith('.pdf');
      });
      if (supportsPdf && editor.viewType) {
        viewers.push({
          extension,
          viewType: editor.viewType,
          priority: editor.priority === 'default' ? 0 : 1
        });
      }
    }
  }
  return viewers.sort((left, right) => left.priority - right.priority);
}

/** Opens a PDF with the first installed VS Code PDF custom editor that works. */
async function openWithInstalledPdfViewer(pdfUri, viewColumn, preserveFocus) {
  for (const viewer of getInstalledPdfViewers()) {
    try {
      if (!viewer.extension.isActive) {
        await viewer.extension.activate();
      }
      await vscode.commands.executeCommand('vscode.openWith', pdfUri, viewer.viewType, {
        viewColumn,
        preview: false,
        preserveFocus
      });
      return true;
    } catch (error) {
      // Try the next registered PDF editor before using the system viewer.
    }
  }
  return false;
}

async function openPdfFile(pdfPath, options = {}) {
  const normalizedPdfPath = path.normalize(pdfPath).toLowerCase();
  const existingGroup = findOpenPdfGroup(pdfPath);
  const forceFocus = options.forceFocus === true;

  // Reopening the custom editor recreates the PDF viewer and loses its
  // split, detached-window, position, and size state.
  if (openedPdfPaths.has(normalizedPdfPath) && !existingGroup) {
    // The PDF may be in a detached VS Code window, which cannot be addressed
    // through the tab API of the compiling window. Do not open a duplicate.
    return true;
  }

  if (!forceFocus && existingGroup) {
    // Remember a PDF found in this window as well, so detaching it afterwards
    // does not make the next compile open a second editor.
    openedPdfPaths.add(normalizedPdfPath);
    return true;
  }

  const pdfExtension = getAcademicPdfViewerExtension();
  if (pdfExtension && !pdfExtension.isActive) {
    try {
      await pdfExtension.activate();
    } catch (error) {
      // Activation failure is non-fatal; the file can still be opened externally.
    }
  }

  const pdfUri = vscode.Uri.file(pdfPath);
  if (pdfExtension) {
    try {
      const viewColumn = existingGroup ? existingGroup.viewColumn : vscode.ViewColumn.Beside;
      await vscode.commands.executeCommand('vscode.openWith', pdfUri, ACADEMIC_PDF_VIEW_TYPE, {
        viewColumn,
        preview: false,
        preserveFocus: options.preserveFocus === true
      });
      openedPdfPaths.add(normalizedPdfPath);
      return true;
    } catch (error) {
      // Fall through to external application.
    }
  }

  const viewColumn = existingGroup ? existingGroup.viewColumn : vscode.ViewColumn.Beside;
  const openedByInstalledViewer = await openWithInstalledPdfViewer(
    pdfUri,
    viewColumn,
    options.preserveFocus === true
  );
  if (!openedByInstalledViewer) {
    await vscode.env.openExternal(pdfUri);
  }
  openedPdfPaths.add(normalizedPdfPath);
  return true;
}

async function reloadActiveAcademicPdf() {
  try {
    // Academic PDF Viewer exposes this command specifically to reload the
    // active PDF.js document while retaining the webview and its view state.
    await vscode.commands.executeCommand(ACADEMIC_PDF_RELOAD_COMMAND);
    return true;
  } catch (error) {
    return false;
  }
}

function isWindowsBatch(commandPath) {
  return process.platform === 'win32' && /\.(cmd|bat)$/i.test(commandPath);
}

/** Runs a process and optionally streams its output to an output channel. */
function runProcess(command, args, cwd, outputChannel = null) {
  return new Promise((resolve) => {
    const child = cp.spawn(command, args, {
      cwd,
      shell: isWindowsBatch(command),
      env: process.env
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (data) => {
      const text = data.toString();
      stdout += text;
      outputChannel?.append(text);
    });
    child.stderr.on('data', (data) => {
      const text = data.toString();
      stderr += text;
      outputChannel?.append(text);
    });
    child.on('error', (error) => resolve({ code: 1, stdout, stderr, error }));
    child.on('close', (code) => resolve({ code: code === null ? 1 : code, stdout, stderr }));
  });
}

/** Resolves the SyncTeX command from the configured TeX tree or PATH. */
function resolveSyncTexExecutable(texRootPath) {
  const root = resolveConfiguredPath(texRootPath);
  if (isExistingDirectory(root)) {
    const resolved = searchForFile(root, process.platform === 'win32'
      ? ['synctex.exe', 'miktex-synctex.exe']
      : ['synctex'], 7);
    if (resolved) {
      return resolved;
    }
  }
  return 'synctex';
}

/** Resolves the ConTeXt mtxrun executable from the configured TeX tree. */
function resolveMtxRunExecutable(texRootPath) {
  const root = resolveConfiguredPath(texRootPath);
  if (!isExistingDirectory(root)) {
    return '';
  }
  const targetNames = process.platform === 'win32'
    ? ['mtxrun.exe', 'mtxrun.cmd', 'mtxrun.bat']
    : ['mtxrun', 'mtxrun.sh', 'mtxrun.lua'];
  return searchForFile(root, targetNames, 8) || '';
}

/** Parses the first SyncTeX view result into a PDF page location. */
function parseSyncTexViewResult(output) {
  const values = {};
  for (const line of String(output || '').split(/\r?\n/)) {
    const match = /^(Page|x|y):\s*(-?\d+(?:\.\d+)?)/i.exec(line.trim());
    if (match) {
      values[match[1].toLowerCase()] = Number(match[2]);
      if (Number.isSafeInteger(values.page) && Number.isFinite(values.x) && Number.isFinite(values.y)) {
        return { pageNumber: values.page, x: values.x, y: values.y };
      }
    }
  }
  return null;
}

/** Parses a SyncTeX edit result into a source file location. */
function parseSyncTexEditResult(output) {
  const locations = [];
  let current = null;

  for (const rawLine of String(output || '').split(/\r?\n/)) {
    const line = rawLine.trim();
    const input = /^Input:\s*(?:\d+:)?(.+)$/i.exec(line);
    if (input) {
      if (current && current.line !== undefined) {
        locations.push(current);
      }
      current = {
        filePath: input[1].trim(),
        line: undefined,
        column: 0
      };
      continue;
    }

    if (!current) {
      continue;
    }
    const sourceLine = /^Line:\s*(-?\d+)$/i.exec(line);
    if (sourceLine) {
      current.line = Math.max(Number(sourceLine[1]), 1);
      continue;
    }
    const column = /^Column:\s*(-?\d+)$/i.exec(line);
    if (column) {
      current.column = Math.max(Number(column[1]), 0);
    }
  }

  if (current && current.line !== undefined) {
    locations.push(current);
  }
  // SyncTeX orders records from the most accurate match to less accurate
  // fallback matches, so the first complete record is the preferred target.
  return locations.length > 0 ? locations[0] : null;
}

/** Parses the ConTeXt mtx-synctex forward result. */
function parseMtxSyncTexFindResult(output) {
  const match = /page\s*=\s*(\d+)\s+llx\s*=\s*(-?\d+(?:\.\d+)?)\s+lly\s*=\s*(-?\d+(?:\.\d+)?)/i.exec(String(output || ''));
  if (!match) {
    return null;
  }
  return {
    pageNumber: Number(match[1]),
    x: Number(match[2]),
    y: Number(match[3])
  };
}

/** Parses the ConTeXt mtx-synctex inverse result. */
function parseMtxSyncTexReportResult(output) {
  const match = /^\s*["'](.+)["']\s+(-?\d+)\s+(-?\d+)\s*$/m.exec(String(output || ''));
  if (!match) {
    return null;
  }
  return {
    filePath: match[1].trim(),
    line: Math.max(Number(match[2]), 1),
    column: Math.max(Number(match[3]), 0)
  };
}

/** Returns the active Academic PDF Viewer API, activating it when necessary. */
async function getAcademicPdfViewerApi() {
  const extension = vscode.extensions.getExtension(ACADEMIC_PDF_VIEWER_EXTENSION_ID);
  if (!extension) {
    return null;
  }
  try {
    const api = extension.isActive ? extension.exports : await extension.activate();
    return api && api.tex && typeof api.tex.synctexForward === 'function'
      && api.tex.onDidRequestInverseSyncTex
      ? api
      : null;
  } catch (error) {
    return null;
  }
}

function activate(context) {
  const academicPdfViewerPromptKey = 'contextIntellisense.academicPdfViewerPromptShown';
  const contextLanguageIds = [
    'context.tex',
    'context.mps',
    'context.lua',
    'context.cld',
    'context.xml',
    'context.bibtex',
    'context.sql',
    'context.bnf',
    'context.cpp',
    'context.pdf',
    'context.json'
  ];

  const outputChannel = vscode.window.createOutputChannel('ConTeXt IntelliSense');
  const fileDecorationsEmitter = new vscode.EventEmitter();
  const codeLensesEmitter = new vscode.EventEmitter();
  const compileDiagnostics = vscode.languages.createDiagnosticCollection('context-intellisense');
  const compileStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  compileStatus.name = 'ConTeXt compilation';
  compileStatus.hide();
  context.subscriptions.push(outputChannel, fileDecorationsEmitter, codeLensesEmitter, compileDiagnostics, compileStatus);

  /** Writes a complete JSON diagnostic record for one SyncTeX operation. */
  function writeSyncTexTrace(direction, details) {
    outputChannel.appendLine(`[SyncTeX ${direction}]`);
    outputChannel.appendLine(JSON.stringify({
      timestamp: new Date().toISOString(),
      direction,
      ...details
    }, null, 2));
  }

  function getWorkspaceRootPath() {
    return vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0
      ? vscode.workspace.workspaceFolders[0].uri.fsPath
      : '';
  }

  /** Returns the configured forward SyncTeX trigger mode. */
  function getSyncTexMode() {
    const configured = String(vscode.workspace
      .getConfiguration('contextIntellisense')
      .get('synctex', 'doubleclick') || '').toLowerCase();
    return ['off', 'doubleclick', 'rightclick'].includes(configured)
      ? configured
      : 'doubleclick';
  }

  function getSyncTexIntegrationMode() {
    const configured = String(vscode.workspace
      .getConfiguration('contextIntellisense')
      .get('synctexmode', 'API') || '').toLowerCase();
    return configured === 'bridge' ? 'bridge' : 'api';
  }

  function getConfiguredTexRootPath() {
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    return String(config.get('texRootPath', '') || '').trim();
  }

  function getConfiguredLegacyXmlPath() {
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    return config.get('xmlPath');
  }

  function getConfiguredMainFilePath() {
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    return String(config.get('mainFilePath', '') || '').trim();
  }

  /** Resolves a workspace-relative path received from the PDF viewer. */
  function resolveWorkspaceRelativePath(relativePath) {
    const workspaceRoot = getWorkspaceRootPath();
    const value = String(relativePath || '').trim();
    if (!workspaceRoot || !value || path.isAbsolute(value)) {
      return '';
    }
    const resolved = path.resolve(workspaceRoot, value);
    return toWorkspaceRelativePath(resolved, workspaceRoot) ? resolved : '';
  }

  /** Resolves a PDF URI from the Academic PDF Viewer API to a local path. */
  function resolveViewerPdfPath(pdfUri) {
    const value = String(pdfUri || '').trim();
    if (!value) {
      return '';
    }

    try {
      const uri = vscode.Uri.parse(value);
      if (uri.scheme === 'file') {
        return uri.fsPath;
      }
      if (uri.scheme) {
        return '';
      }
    } catch (error) {
      return '';
    }

    return path.isAbsolute(value) ? value : resolveWorkspaceRelativePath(value);
  }

  function hasConfiguredTexRoot() {
    return String(getConfiguredTexRootPath() || '').trim().length > 0;
  }

  function getResolvedTexRootPath() {
    return resolveTexRootPath(getConfiguredTexRootPath(), getConfiguredLegacyXmlPath());
  }

  function getResolvedXmlPath() {
    const texRootPath = getResolvedTexRootPath();
    return resolveXmlPathFromTexRoot(texRootPath) || resolveXmlPath(getConfiguredLegacyXmlPath());
  }

  function getResolvedContextExecutable() {
    const texRootPath = getResolvedTexRootPath();
    return texRootPath ? resolveContextExecutableFromTexRoot(texRootPath) : '';
  }

  function getResolvedMainFilePath() {
    return resolveMainFilePath(getConfiguredMainFilePath(), getWorkspaceRootPath());
  }

  /** Finds the SyncTeX sidecar associated with a generated PDF. */
  function findSyncTexSidecar(pdfPath) {
    const stem = pdfPath.slice(0, -path.extname(pdfPath).length);
    const candidates = [
      `${stem}.synctex`,
      `${stem}.synctex.gz`,
      `${stem}.synctex(busy)`
    ];
    return candidates.find(isExistingFile) || '';
  }

  async function ensureGzippedSyncTexSidecar(pdfPath) {
    const plainPath = `${pdfPath.slice(0, -path.extname(pdfPath).length)}.synctex`;
    const compressedPath = `${plainPath}.gz`;
    if (!isExistingFile(plainPath)) {
      return false;
    }
    const content = await fs.promises.readFile(plainPath);
    await fs.promises.writeFile(compressedPath, zlib.gzipSync(content));
    return true;
  }

  async function configureAcademicPdfViewerBridge(pdfPath) {
    const config = vscode.workspace.getConfiguration('academicPdfViewer');
    await config.update('tex.bridge.enabled', true, vscode.ConfigurationTarget.Workspace);
    await config.update('tex.bridge.executable', resolveSyncTexExecutable(getResolvedTexRootPath()), vscode.ConfigurationTarget.Workspace);
    await config.update('tex.bridge.pdfPath', pdfPath, vscode.ConfigurationTarget.Workspace);
  }

  /** Returns the source name exactly as recorded in a SyncTeX sidecar. */
  function resolveSyncTexInputName(sourcePath, sidecarPath, pdfPath) {
    let content;
    try {
      const data = fs.readFileSync(sidecarPath);
      content = sidecarPath.toLowerCase().endsWith('.gz')
        ? zlib.gunzipSync(data).toString('utf8')
        : data.toString('utf8');
    } catch (error) {
      return sourcePath;
    }

    const normalizedSourcePath = path.normalize(sourcePath).toLowerCase();
    for (const line of content.split(/\r?\n/)) {
      const match = /^Input:\d+:(.+)$/i.exec(line.trim());
      if (!match) {
        continue;
      }
      const inputName = match[1].trim();
      const inputPath = path.isAbsolute(inputName)
        ? inputName
        : path.resolve(path.dirname(pdfPath), inputName);
      if (path.normalize(inputPath).toLowerCase() === normalizedSourcePath) {
        return inputName;
      }
    }
    return sourcePath;
  }

  /** Returns the workspace root when the PDF belongs to the active workspace. */
  function getSyncTexWorkingDirectory(pdfPath) {
    const workspaceRoot = getWorkspaceRootPath();
    if (workspaceRoot) {
      const relativePath = path.relative(workspaceRoot, pdfPath);
      if (relativePath && !relativePath.startsWith(`..${path.sep}`) && !path.isAbsolute(relativePath)) {
        return workspaceRoot;
      }
    }
    return path.dirname(pdfPath);
  }

  /** Converts a file path to a normalized path relative to the SyncTeX working directory. */
  function toSyncTexRelativePath(filePath, workingDirectory, relativeBase = workingDirectory) {
    const absolutePath = path.isAbsolute(filePath)
      ? filePath
      : path.resolve(relativeBase, filePath);
    const relativePath = path.relative(workingDirectory, absolutePath);
    return (relativePath || path.basename(absolutePath)).split(path.sep).join('/');
  }

  /** Resolves a SyncTeX source path relative to the generated PDF. */
  function resolveSyncTexSourcePath(sourcePath, pdfPath) {
    const value = String(sourcePath || '').trim();
    if (!value) {
      return '';
    }
    const unquoted = value.replace(/^['"]|['"]$/g, '');
    const candidates = [
      path.isAbsolute(unquoted) ? unquoted : path.resolve(path.dirname(pdfPath), unquoted),
      path.isAbsolute(unquoted) ? unquoted : path.resolve(getWorkspaceRootPath(), unquoted)
    ];
    return candidates.find(isExistingFile) || candidates[0];
  }

  /** Resolves the PDF produced by the configured main ConTeXt document. */
  function resolveSyncTexPdfPath(sourcePath) {
    const mainFilePath = getResolvedMainFilePath();
    const basePath = isExistingFile(mainFilePath) ? mainFilePath : sourcePath;
    const pdfPath = path.join(
      path.dirname(basePath),
      `${path.basename(basePath, path.extname(basePath))}.pdf`
    );
    return isExistingFile(pdfPath) && findSyncTexSidecar(pdfPath) ? pdfPath : '';
  }

  /** Builds the preferred ConTeXt SyncTeX command and its arguments. */
  function buildSyncTexCommand(direction, sourcePath, pdfPath, sidecarPath, selection, event) {
    const cwd = getSyncTexWorkingDirectory(pdfPath);
    const mtxrun = resolveMtxRunExecutable(getResolvedTexRootPath());
    if (mtxrun) {
      const sidecarName = toSyncTexRelativePath(sidecarPath, cwd);
      if (direction === 'forward') {
        const sourceName = toSyncTexRelativePath(sourcePath, cwd);
        return {
          executable: mtxrun,
          args: [
            '--script', 'synctex', '--find',
            `--file=${sourceName}`,
            `--line=${selection.active.line + 1}`,
            sidecarName
          ],
          backend: 'mtxrun',
          cwd
        };
      }
      return {
        executable: mtxrun,
        args: [
          '--script', 'synctex', '--report',
          `--page=${event.pageNumber}`,
          `--x=${event.x}`,
          `--y=${event.y}`,
          '--console',
          sidecarName
        ],
        backend: 'mtxrun',
        cwd
      };
    }

    if (direction === 'forward') {
      const recordedInputName = resolveSyncTexInputName(sourcePath, sidecarPath, pdfPath);
      const inputName = path.isAbsolute(recordedInputName)
        ? recordedInputName.split(path.sep).join('/')
        : recordedInputName.replace(/\\/g, '/');
      const relativePdfPath = toSyncTexRelativePath(pdfPath, cwd);
      return {
        executable: resolveSyncTexExecutable(getResolvedTexRootPath()),
        args: [
          'view',
          '-i', `${selection.active.line + 1}:${selection.active.character}:${inputName}`,
          '-o', relativePdfPath,
          '-d', toSyncTexRelativePath(path.dirname(sidecarPath), cwd)
        ],
        backend: 'synctex',
        cwd
      };
    }
    return {
      executable: resolveSyncTexExecutable(getResolvedTexRootPath()),
      args: [
        'edit',
        '-o', `${event.pageNumber}:${event.x}:${event.y}:${toSyncTexRelativePath(pdfPath, cwd)}`,
        '-d', toSyncTexRelativePath(path.dirname(sidecarPath), cwd)
      ],
      backend: 'synctex',
      cwd
    };
  }

  /** Runs SyncTeX forward search for an editor position and sends its JSON location to the viewer. */
  async function forwardSyncTex(editor, options = {}) {
    const document = editor && editor.document;
    if (!document || document.languageId !== 'context.tex') {
      return;
    }

    if (getSyncTexIntegrationMode() === 'bridge') {
      if (getSyncTexMode() === 'off') {
        return;
      }
      try {
        const accepted = await vscode.commands.executeCommand('academicPdfViewer.tex.synctexForwardFromCursor');
        if (accepted !== false) {
          return;
        }
        outputChannel.appendLine('Academic PDF Viewer bridge returned no location; using ConTeXt/API forward search.');
      } catch (error) {
        outputChannel.appendLine(`Academic PDF Viewer bridge forward search failed: ${error.message || error}`);
      }
      return;
    }

    const selection = editor.selection;
    const requestDetails = {
      editorUri: formatSyncTexTraceUri(document.uri.toString()),
      documentPath: formatSyncTexTracePath(document.uri.fsPath),
      languageId: document.languageId,
      selection: selection ? {
        isEmpty: selection.isEmpty,
        anchor: { line: selection.anchor.line, character: selection.anchor.character },
        active: { line: selection.active.line, character: selection.active.character }
      } : null
    };
    const allowEmptySelection = options.allowEmpty === true;
    requestDetails.trigger = options.trigger || 'unknown';
    requestDetails.allowEmptySelection = allowEmptySelection;
    if (!selection || ((!allowEmptySelection && selection.isEmpty)
      || selection.start.line !== selection.end.line)) {
      writeSyncTexTrace('forward', {
        ...requestDetails,
        trigger: options.trigger || 'unknown',
        status: 'ignored',
        reason: allowEmptySelection ? 'Selection spans multiple lines.' : 'No single-line selection.'
      });
      return;
    }

    const pdfPath = resolveSyncTexPdfPath(document.uri.fsPath);
    if (!pdfPath) {
      writeSyncTexTrace('forward', {
        ...requestDetails,
        status: 'ignored',
        reason: 'No main PDF or SyncTeX sidecar found.',
        configuredMainFile: formatSyncTexTracePath(getConfiguredMainFilePath()),
        resolvedMainFile: formatSyncTexTracePath(getResolvedMainFilePath())
      });
      return;
    }

    const sidecarPath = findSyncTexSidecar(pdfPath);
    const command = buildSyncTexCommand('forward', document.uri.fsPath, pdfPath, sidecarPath, selection);
    const { executable, args } = command;
    const result = await runProcess(
      executable,
      args,
      command.cwd
    );
    const trace = {
      ...requestDetails,
      status: result.code === 0 ? 'completed' : 'failed',
      executable: formatSyncTexTracePath(executable),
      args,
      cwd: formatSyncTexTracePath(command.cwd),
      configuredTexRoot: formatSyncTexTracePath(getConfiguredTexRootPath()),
      resolvedTexRoot: formatSyncTexTracePath(getResolvedTexRootPath()),
      pdfPath: formatSyncTexTracePath(pdfPath),
      sidecarPath: formatSyncTexTracePath(sidecarPath),
      backend: command.backend,
      exitCode: result.code,
      stdout: result.stdout,
      stderr: result.stderr
    };
    if (result.code !== 0) {
      writeSyncTexTrace('forward', trace);
      outputChannel.appendLine(`SyncTeX forward search failed: ${result.stderr || result.stdout}`);
      return;
    }

    const location = command.backend === 'mtxrun'
      ? parseMtxSyncTexFindResult(result.stdout)
      : parseSyncTexViewResult(result.stdout);
    if (!location) {
      writeSyncTexTrace('forward', { ...trace, status: 'no-location', parsedLocation: null });
      outputChannel.appendLine(`SyncTeX forward search returned no PDF location. Output: ${result.stdout || result.stderr}`);
      return;
    }

    const viewer = await getAcademicPdfViewerApi();
    if (!viewer) {
      writeSyncTexTrace('forward', { ...trace, parsedLocation: location, status: 'viewer-api-unavailable' });
      outputChannel.appendLine('Academic PDF Viewer SyncTeX API is not available.');
      return;
    }

    const message = {
      type: 'synctex.forward',
      pdfUri: vscode.Uri.file(pdfPath).toString(),
      pageNumber: location.pageNumber,
      x: location.x,
      y: location.y
    };
    const accepted = viewer.tex.synctexForward(message);
    writeSyncTexTrace('forward', {
      ...trace,
      parsedLocation: location,
      viewerMessage: { ...message, pdfUri: formatSyncTexTraceUri(message.pdfUri) },
      viewerAccepted: accepted
    });
  }

  /** Finds a visible text editor for a normalized source path. */
  function findVisibleSourceEditor(sourcePath) {
    const normalizedSourcePath = path.normalize(sourcePath).toLowerCase();
    return vscode.window.visibleTextEditors.find((editor) => {
      return editor.document.uri.scheme === 'file'
        && path.normalize(editor.document.uri.fsPath).toLowerCase() === normalizedSourcePath;
    }) || null;
  }

  /** Finds an already open source tab and returns its editor column. */
  function findOpenSourceTab(sourcePath) {
    const normalizedSourcePath = path.normalize(sourcePath).toLowerCase();
    for (const group of vscode.window.tabGroups.all) {
      for (const tab of group.tabs) {
        const uri = tab.input && tab.input.uri;
        if (uri && uri.scheme === 'file'
          && path.normalize(uri.fsPath).toLowerCase() === normalizedSourcePath) {
          return { viewColumn: group.viewColumn };
        }
      }
    }
    return null;
  }

  /** Selects an existing normal editor group without targeting the PDF group. */
  function findNormalEditorColumn(pdfPath) {
    const pdfGroup = findOpenPdfGroup(pdfPath);
    const normalGroup = vscode.window.tabGroups.all.find((group) => {
      if (pdfGroup && group === pdfGroup) {
        return false;
      }
      return group.tabs.some((tab) => {
        const uri = tab.input && tab.input.uri;
        return uri && uri.scheme === 'file'
          && path.extname(uri.fsPath).toLowerCase() !== '.pdf';
      });
    });
    return normalGroup ? normalGroup.viewColumn : vscode.ViewColumn.Beside;
  }

  /** Opens or reuses a source editor in a normal editor group. */
  async function showSyncTexSource(document, sourcePath, pdfPath) {
    const visibleEditor = findVisibleSourceEditor(sourcePath);
    const openSourceTab = findOpenSourceTab(sourcePath);
    const viewColumn = visibleEditor
      ? visibleEditor.viewColumn
      : openSourceTab
        ? openSourceTab.viewColumn
        : findNormalEditorColumn(pdfPath);

    return vscode.window.showTextDocument(document, {
      viewColumn,
      preview: false,
      preserveFocus: false
    });
  }

  /** Resolves an inverse SyncTeX event and reveals the corresponding source position. */
  async function handleInverseSyncTex(event) {
    if (getSyncTexIntegrationMode() !== 'api') {
      return;
    }
    if (getSyncTexMode() === 'off') {
      writeSyncTexTrace('inverse', {
        status: 'ignored',
        reason: 'SyncTeX is disabled by contextIntellisense.synctex.',
        viewerEvent: { ...event, pdfUri: formatSyncTexTraceUri(event.pdfUri) }
      });
      return;
    }
    const pdfPath = resolveViewerPdfPath(event.pdfUri);
    const sidecarPath = findSyncTexSidecar(pdfPath);
    const requestDetails = {
      viewerEvent: { ...event, pdfUri: formatSyncTexTraceUri(event.pdfUri) },
      pdfUri: formatSyncTexTraceUri(event.pdfUri),
      pdfPath: formatSyncTexTracePath(pdfPath),
      pdfExists: isExistingFile(pdfPath),
      sidecarPath: formatSyncTexTracePath(sidecarPath),
      sidecarExists: !!sidecarPath
    };
    if (!isExistingFile(pdfPath) || !sidecarPath) {
      writeSyncTexTrace('inverse', { ...requestDetails, status: 'ignored', reason: 'PDF or SyncTeX sidecar not found.' });
      return;
    }

    const command = buildSyncTexCommand('inverse', '', pdfPath, sidecarPath, null, event);
    const { executable, args } = command;
    const result = await runProcess(
      executable,
      args,
      command.cwd
    );
    const trace = {
      ...requestDetails,
      executable: formatSyncTexTracePath(executable),
      args,
      cwd: formatSyncTexTracePath(command.cwd),
      configuredTexRoot: formatSyncTexTracePath(getConfiguredTexRootPath()),
      resolvedTexRoot: formatSyncTexTracePath(getResolvedTexRootPath()),
      backend: command.backend,
      exitCode: result.code,
      stdout: result.stdout,
      stderr: result.stderr
    };
    if (result.code !== 0) {
      writeSyncTexTrace('inverse', { ...trace, status: 'failed' });
      outputChannel.appendLine(`SyncTeX inverse search failed: ${result.stderr || result.stdout}`);
      return;
    }

    const location = command.backend === 'mtxrun'
      ? parseMtxSyncTexReportResult(result.stdout)
      : parseSyncTexEditResult(result.stdout);
    if (!location) {
      writeSyncTexTrace('inverse', { ...trace, status: 'no-location', parsedLocation: null });
      outputChannel.appendLine('SyncTeX inverse search returned no source location.');
      return;
    }

    const sourcePath = resolveSyncTexSourcePath(location.filePath, pdfPath);
    if (!isExistingFile(sourcePath)) {
      writeSyncTexTrace('inverse', {
        ...trace,
        status: 'source-not-found',
        parsedLocation: { ...location, filePath: formatSyncTexTracePath(location.filePath) },
        resolvedSourcePath: formatSyncTexTracePath(sourcePath)
      });
      outputChannel.appendLine(`SyncTeX source file not found: ${location.filePath}`);
      return;
    }

    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(sourcePath));
    const editor = await showSyncTexSource(document, sourcePath, pdfPath);
    const line = Math.min(location.line - 1, Math.max(document.lineCount - 1, 0));
    const character = Math.min(location.column, document.lineAt(line).text.length);
    const position = new vscode.Position(line, character);
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
    writeSyncTexTrace('inverse', {
      ...trace,
      status: 'completed',
      parsedLocation: { ...location, filePath: formatSyncTexTracePath(location.filePath) },
      resolvedSourcePath: formatSyncTexTracePath(sourcePath),
      editorUri: formatSyncTexTraceUri(toWorkspaceRelativePath(sourcePath, getWorkspaceRootPath())),
      editorPosition: { line: line + 1, character }
    });
  }

  /** Subscribes to the Academic PDF Viewer's inverse SyncTeX event. */
  async function connectAcademicPdfViewerSyncTex() {
    if (getSyncTexIntegrationMode() !== 'api') {
      return;
    }
    const viewer = await getAcademicPdfViewerApi();
    if (!viewer) {
      return;
    }
    context.subscriptions.push(viewer.tex.onDidRequestInverseSyncTex((event) => {
      void handleInverseSyncTex(event).catch((error) => {
        writeSyncTexTrace('inverse', {
          status: 'exception',
          viewerEvent: { ...event, pdfUri: formatSyncTexTraceUri(event.pdfUri) },
          error: String(error && error.stack ? error.stack : error)
        });
      });
    }));
  }

  function refreshDecorations() {
    fileDecorationsEmitter.fire();
    codeLensesEmitter.fire();
  }

  async function configureTexRootPathWithPicker() {
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: 'Use this folder',
      title: 'Choose ConTeXt distribution tex tree folder'
    });

    if (!selected || selected.length === 0) {
      return false;
    }

    const texRootPath = selected[0].fsPath;
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    await config.update('texRootPath', texRootPath, vscode.ConfigurationTarget.Global);
    loadData(texRootPath, getConfiguredLegacyXmlPath());
    await applyContextLanguageToOpenDocuments();
    refreshDecorations();

    const resolvedXmlPath = getResolvedXmlPath();
    if (!resolvedXmlPath) {
      vscode.window.showWarningMessage(`ConTeXt IntelliSense configured with TeX root, but i-context.xml was not found below: ${texRootPath}`);
    } else {
      vscode.window.showInformationMessage(`ConTeXt IntelliSense configured with TeX root: ${texRootPath}`);
    }
    await maybeShowAcademicPdfViewerPrompt();
    return true;
  }

  async function maybeShowAcademicPdfViewerPrompt() {
    if (context.globalState.get(academicPdfViewerPromptKey, false)) {
      return;
    }

    // The prompt is only useful when the optional viewer is not installed.
    if (getAcademicPdfViewerExtension()) {
      await context.globalState.update(academicPdfViewerPromptKey, true);
      return;
    }

    await context.globalState.update(academicPdfViewerPromptKey, true);
    const openLabel = 'Open Academic PDF Viewer';
    const selection = await vscode.window.showInformationMessage(
      'For the best integrated PDF workflow, ConTeXt IntelliSense works optimally with the Academic PDF Viewer extension.',
      openLabel,
      'Not now'
    );

    if (selection !== openLabel) {
      return;
    }

    const marketplaceSearch = '@id:ovolab-veritas.academic-pdf-viewer';
    try {
      await vscode.commands.executeCommand('workbench.extensions.search', marketplaceSearch);
    } catch (error) {
      await vscode.env.openExternal(vscode.Uri.parse(
        'https://marketplace.visualstudio.com/items?itemName=ovolab-veritas.academic-pdf-viewer'
      ));
    }
  }

  async function configureMainFilePathWithPicker() {
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      filters: {
        TeX: ['tex', 'mkiv', 'mkxl', 'mkvi', 'mkil', 'mkix', 'mkxi', 'mklx']
      },
      openLabel: 'Use this file',
      title: 'Select the main ConTeXt file'
    });

    if (!selected || selected.length === 0) {
      return false;
    }

    const workspaceRootPath = getWorkspaceRootPath();
    const mainFilePath = toWorkspaceRelativePath(selected[0].fsPath, workspaceRootPath);
    if (!mainFilePath) {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: the main file must be inside the current workspace.');
      return false;
    }
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    await config.update('mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
    refreshDecorations();
    vscode.window.showInformationMessage(`ConTeXt IntelliSense main file set to: ${mainFilePath}`);
    return true;
  }

  async function maybeRunFirstStartSetup(force = false) {
    const configuredTexRootPath = getConfiguredTexRootPath();
    if (hasConfiguredTexRoot() && !force) {
      return;
    }

    const actionChoose = 'Choose ConTeXt distribution tex tree folder';
    const actionNotNow = 'Not now';

    const selection = await vscode.window.showInformationMessage(
      'ConTeXt IntelliSense needs your ConTeXt distribution tex tree folder for completions, signatures, and compile commands.',
      actionChoose,
      actionNotNow
    );

    if (selection !== actionChoose) {
      return;
    }

    await configureTexRootPathWithPicker();
  }

  /** Converts a ConTeXt error log into navigable VS Code diagnostics. */
  function readCompileDiagnostics(errorLogPath, fallbackFilePath) {
    if (!isExistingFile(errorLogPath)) {
      return [];
    }

    const diagnostics = [];
    const logText = fs.readFileSync(errorLogPath, 'utf8');
    const structuredFile = logText.match(/\["filename"\]\s*=\s*"([^"]+)"/);
    const structuredLine = logText.match(/\["linenumber"\]\s*=\s*(\d+)/);
    const structuredError = logText.match(/\["lasttexerror"\]\s*=\s*"([^"]+)"/)
      || logText.match(/\["lastluaerror"\]\s*=\s*"([^"]+)"/);
    const structuredHelp = logText.match(/\["lasttexhelp"\]\s*=\s*"([^"]*)"/);
    if (structuredError && structuredError[1]) {
      const filePath = structuredFile ? structuredFile[1] : fallbackFilePath;
      const lineNumber = structuredLine ? Number(structuredLine[1]) : 1;
      const help = structuredHelp && structuredHelp[1] ? ` ${structuredHelp[1].replace(/\\n/g, ' ')}` : '';
      const uri = vscode.Uri.file(resolveConfiguredPath(filePath, path.dirname(fallbackFilePath)));
      const line = Math.max(lineNumber - 1, 0);
      diagnostics.push({
        uri,
        diagnostic: new vscode.Diagnostic(
          new vscode.Range(line, 0, line, 0),
          `${structuredError[1].trim()}${help}`,
          vscode.DiagnosticSeverity.Error
        )
      });
      return diagnostics;
    }

    const lines = logText.split(/\r?\n/);
    let message = '';
    let lineNumber = 1;
    let filePath = fallbackFilePath;
    const addDiagnostic = () => {
      if (!message) {
        return;
      }
      const uri = vscode.Uri.file(resolveConfiguredPath(filePath, path.dirname(fallbackFilePath)));
      const range = new vscode.Range(Math.max(lineNumber - 1, 0), 0, Math.max(lineNumber - 1, 0), 0);
      diagnostics.push({ uri, diagnostic: new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Error) });
      message = '';
    };

    for (const line of lines) {
      const error = line.match(/^!\s*(.+)$/);
      if (error) {
        addDiagnostic();
        message = error[1].trim();
      }
      const source = line.match(/(?:file|source)\s*[:=]\s*(.+?\.(?:tex|mkiv|mkvi|mkxl|mklx))(?:\s|$)/i);
      if (source) {
        filePath = source[1].trim();
      }
      const location = line.match(/\b(?:l(?:ine)?\.?)[\s:]*(\d+)\b/i);
      if (location) {
        lineNumber = Number(location[1]);
      }
    }
    addDiagnostic();
    return diagnostics;
  }

  /** Publishes compiler diagnostics and returns a compact user-facing summary. */
  function publishCompileDiagnostics(errorLogPath, targetFilePath, fallbackMessage) {
    compileDiagnostics.clear();
    const diagnostics = readCompileDiagnostics(errorLogPath, targetFilePath);
    const grouped = new Map();
    for (const item of diagnostics) {
      if (!grouped.has(item.uri.toString())) {
        grouped.set(item.uri.toString(), []);
      }
      grouped.get(item.uri.toString()).push(item.diagnostic);
    }
    for (const item of diagnostics) {
      compileDiagnostics.set(item.uri, grouped.get(item.uri.toString()));
    }
    if (diagnostics.length === 0) {
      return { summary: fallbackMessage, diagnostics };
    }
    const first = diagnostics[0];
    const line = first.diagnostic.range.start.line + 1;
    return {
      summary: `${diagnostics.length} error${diagnostics.length === 1 ? '' : 's'} found: ${first.diagnostic.message} (${path.basename(first.uri.fsPath)}:${line})`,
      diagnostics
    };
  }

  /** Detects a ConTeXt error even when the process exits successfully. */
  function hasCompileErrorLog(errorLogPath) {
    if (!isExistingFile(errorLogPath)) {
      return false;
    }
    const text = fs.readFileSync(errorLogPath, 'utf8');
    return /\["last(?:tex|lua)error"\]\s*=\s*"[^"]+"/.test(text)
      || /^!/m.test(text);
  }

  async function compileTargetFile(targetFilePath, title) {
    const resolvedTargetPath = resolveConfiguredPath(targetFilePath, getWorkspaceRootPath());
    if (!isExistingFile(resolvedTargetPath)) {
      vscode.window.showErrorMessage(`ConTeXt IntelliSense: target file not found: ${resolvedTargetPath}`);
      return false;
    }

    const contextExecutable = getResolvedContextExecutable();
    if (!contextExecutable) {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: could not locate the ConTeXt executable. Set the TeX root folder first.');
      return false;
    }

    const cwd = path.dirname(resolvedTargetPath);
    const pdfPath = path.join(cwd, `${path.basename(resolvedTargetPath, path.extname(resolvedTargetPath))}.pdf`);
    const errorLogPath = path.join(cwd, `${path.basename(resolvedTargetPath, path.extname(resolvedTargetPath))}-error.log`);

    compileDiagnostics.delete(vscode.Uri.file(resolvedTargetPath));
    compileStatus.text = '$(sync~spin) ConTeXt: compiling';
    compileStatus.tooltip = `Compiling ${path.basename(resolvedTargetPath)}`;
    compileStatus.show();
    outputChannel.clear();
    outputChannel.appendLine(`ConTeXt IntelliSense: ${title}`);
    outputChannel.appendLine(`Working directory: ${cwd}`);
    const compileArgs = ['--synctex', resolvedTargetPath];
    outputChannel.appendLine(`Command: ${contextExecutable} ${compileArgs.join(' ')}`);
    let compilationFailure = null;
    try {
      await vscode.window.withProgress({
        location: vscode.ProgressLocation.Window,
        title,
        cancellable: false
      }, async () => {
        const result = await runProcess(contextExecutable, compileArgs, cwd, outputChannel);
        const logHasError = hasCompileErrorLog(errorLogPath);
        if (result.error || result.code !== 0 || logHasError) {
          const reason = result.error
            ? result.error.message || String(result.error)
            : result.code !== 0
              ? `exit code ${result.code}`
              : 'the ConTeXt error log reports an error';
          outputChannel.appendLine(`Compilation failed: ${reason}`);
          compilationFailure = publishCompileDiagnostics(errorLogPath, resolvedTargetPath, reason);
          return;
        }

        compileDiagnostics.clear();
        outputChannel.appendLine('Compilation completed successfully.');
        vscode.window.setStatusBarMessage(
          `$(check) ConTeXt compilation succeeded: ${path.basename(resolvedTargetPath)}`,
          4000
        );
        if (getSyncTexIntegrationMode() === 'bridge') {
          try {
            const compressed = await ensureGzippedSyncTexSidecar(pdfPath);
            if (compressed) {
              await configureAcademicPdfViewerBridge(pdfPath);
              outputChannel.appendLine(`SyncTeX bridge configured for: ${pdfPath}`);
            } else {
              outputChannel.appendLine(`SyncTeX bridge sidecar not found: ${pdfPath}`);
            }
          } catch (error) {
            outputChannel.appendLine(`SyncTeX bridge setup failed: ${error.message || error}`);
          }
        }
        if (isExistingFile(pdfPath)) {
          await reloadActiveAcademicPdf();
          const openPdfAfterCompile = vscode.workspace
            .getConfiguration('contextIntellisense')
            .get('openPdfAfterCompile', true);
          if (openPdfAfterCompile) {
            await openPdfFile(pdfPath);
          }
        } else {
          vscode.window.showWarningMessage(`ConTeXt compilation succeeded, but no PDF was found at: ${pdfPath}`);
        }
      });
    } finally {
      compileStatus.hide();
    }

    if (compilationFailure) {
      await vscode.commands.executeCommand('workbench.actions.view.problems');
    }

    return true;
  }

  async function compileActiveOrProvidedDocument(targetUri) {
    const uri = targetUri || (vscode.window.activeTextEditor && vscode.window.activeTextEditor.document.uri);
    if (!uri) {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: no active document to compile.');
      return false;
    }

    const document = await vscode.workspace.openTextDocument(uri);
    if (document.languageId !== 'context.tex') {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: compile is only available for ConTeXt TEX files.');
      return false;
    }

    if (document.isDirty) {
      const saved = await document.save();
      if (!saved) {
        return false;
      }
    }

    return compileTargetFile(document.uri.fsPath, 'Compile current ConTeXt file');
  }

  async function compileMainFile() {
    const configuredMainPathRaw = String(getConfiguredMainFilePath() || '').trim();
    const mainFilePath = getResolvedMainFilePath();
    if (!mainFilePath) {
      if (configuredMainPathRaw) {
        const attemptedPath = resolveConfiguredPath(configuredMainPathRaw, getWorkspaceRootPath());
        vscode.window.showErrorMessage(`ConTeXt IntelliSense: configured main file not found: ${attemptedPath}`);
        return false;
      }

      vscode.window.showErrorMessage('ConTeXt IntelliSense: no main file configured. Use "Set as Main File" in Explorer or "Configure Main File" in the command palette.');
      return false;
    }

    for (const document of vscode.workspace.textDocuments) {
      if (document.languageId === 'context.tex' && document.isDirty && !await document.save()) {
        vscode.window.showErrorMessage(`ConTeXt IntelliSense: could not save ${document.fileName}.`);
        return false;
      }
    }

    return compileTargetFile(mainFilePath, 'Compile main ConTeXt file');
  }

  async function showPdfForActiveDocument() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'context.tex') {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: no active ConTeXt TEX document to show a PDF for.');
      return false;
    }

    const pdfPath = path.join(
      path.dirname(editor.document.uri.fsPath),
      `${path.basename(editor.document.uri.fsPath, path.extname(editor.document.uri.fsPath))}.pdf`
    );

    if (!isExistingFile(pdfPath)) {
      vscode.window.showWarningMessage(`ConTeXt IntelliSense: PDF not found: ${pdfPath}`);
      return false;
    }

    await openPdfFile(pdfPath, { forceFocus: true });
    return true;
  }

  async function clearWorkspaceTemporaryFiles() {
    const workspaceFolders = vscode.workspace.workspaceFolders || [];
    if (workspaceFolders.length === 0) {
      vscode.window.showWarningMessage('ConTeXt IntelliSense: no workspace folder is open.');
      return false;
    }

    const filesToDelete = collectWorkspaceTemporaryFiles(workspaceFolders);
    if (filesToDelete.length === 0) {
      vscode.window.showInformationMessage('ConTeXt IntelliSense: no temporary files found.');
      return true;
    }

    const confirmLabel = `Delete ${filesToDelete.length} files`;
    const selection = await vscode.window.showWarningMessage(
      `Delete ${filesToDelete.length} temporary ConTeXt files from the workspace and its subfolders?`,
      { modal: true },
      confirmLabel
    );
    if (selection !== confirmLabel) {
      return false;
    }

    const failures = [];
    for (const filePath of filesToDelete) {
      try {
        await fs.promises.unlink(filePath);
      } catch (error) {
        failures.push({ filePath, error });
      }
    }

    const deletedCount = filesToDelete.length - failures.length;
    if (failures.length > 0) {
      outputChannel.appendLine(`Workspace cleanup deleted ${deletedCount} files; ${failures.length} could not be deleted:`);
      for (const failure of failures) {
        outputChannel.appendLine(`${failure.filePath}: ${failure.error.message || failure.error}`);
      }
      vscode.window.showWarningMessage(`ConTeXt IntelliSense: deleted ${deletedCount} files; ${failures.length} failed. See the output channel.`);
      return false;
    }

    vscode.window.showInformationMessage(`ConTeXt IntelliSense: deleted ${deletedCount} temporary files.`);
    return true;
  }

  loadData(getConfiguredTexRootPath(), getConfiguredLegacyXmlPath());
  void applyContextLanguageToOpenDocuments();
  const configureTexRootCommand = vscode.commands.registerCommand('contextIntellisense.configureTexRootPath', async () => {
    await maybeRunFirstStartSetup(true);
  });
  context.subscriptions.push(configureTexRootCommand);

  const configureLegacyXmlCommand = vscode.commands.registerCommand('contextIntellisense.configureXmlPath', async () => {
    await maybeRunFirstStartSetup(true);
  });
  context.subscriptions.push(configureLegacyXmlCommand);

  const configureMainFileCommand = vscode.commands.registerCommand('contextIntellisense.configureMainFilePath', async () => {
    await configureMainFilePathWithPicker();
  });
  context.subscriptions.push(configureMainFileCommand);

  const setMainFromExplorerCommand = vscode.commands.registerCommand('contextIntellisense.setMainFileFromExplorer', async (targetUri, selectedUris) => {
    const candidateUri = (targetUri && targetUri.scheme === 'file')
      ? targetUri
      : (Array.isArray(selectedUris) && selectedUris.length > 0
        ? selectedUris[0]
        : (vscode.window.activeTextEditor ? vscode.window.activeTextEditor.document.uri : null));

    if (!candidateUri || candidateUri.scheme !== 'file') {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: please select a ConTeXt TEX file in Explorer.');
      return;
    }

    if (!isConTeXtTexFilePath(candidateUri.fsPath)) {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: selected file is not a ConTeXt TEX file.');
      return;
    }

    const mainFilePath = toWorkspaceRelativePath(candidateUri.fsPath, getWorkspaceRootPath());
    if (!mainFilePath) {
      vscode.window.showErrorMessage('ConTeXt IntelliSense: the main file must be inside the current workspace.');
      return;
    }
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    await config.update('mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
    refreshDecorations();
    vscode.window.showInformationMessage(`ConTeXt IntelliSense main file set to: ${mainFilePath}`);
  });
  context.subscriptions.push(setMainFromExplorerCommand);

  const compileCurrentCommand = vscode.commands.registerCommand('contextIntellisense.compileCurrentFile', async (targetUri) => {
    await compileActiveOrProvidedDocument(targetUri);
  });
  context.subscriptions.push(compileCurrentCommand);

  const compileMainCommand = vscode.commands.registerCommand('contextIntellisense.compileMainFile', async () => {
    await compileMainFile();
  });
  context.subscriptions.push(compileMainCommand);

  const showPdfCommand = vscode.commands.registerCommand('contextIntellisense.showPdfForCurrentFile', async () => {
    await showPdfForActiveDocument();
  });
  context.subscriptions.push(showPdfCommand);

  const synctexForwardCommand = vscode.commands.registerCommand('contextIntellisense.synctexForward', async () => {
    const mode = getSyncTexMode();
    if (mode !== 'rightclick' && getSyncTexIntegrationMode() !== 'bridge') {
      return;
    }
    await forwardSyncTex(vscode.window.activeTextEditor, {
      allowEmpty: true,
      trigger: mode
    });
  });
  context.subscriptions.push(synctexForwardCommand);

  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection((event) => {
    if (event.kind !== vscode.TextEditorSelectionChangeKind.Mouse) {
      return;
    }

    const mode = getSyncTexMode();
    if (mode !== 'doubleclick') {
      return;
    }

    void forwardSyncTex(event.textEditor, {
      trigger: 'doubleclick'
    }).catch((error) => {
      writeSyncTexTrace('forward', {
        status: 'exception',
        editorUri: event.textEditor && event.textEditor.document.uri.toString(),
        error: String(error && error.stack ? error.stack : error)
      });
    });
  }));

  const clearWorkspaceCommand = vscode.commands.registerCommand('contextIntellisense.clearWorkspace', async () => {
    await clearWorkspaceTemporaryFiles();
  });
  context.subscriptions.push(clearWorkspaceCommand);

  context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(() => {
    void applyContextLanguageToOpenDocuments();
  }));

  // A PDF that was closed must be allowed to open again. Without this,
  // openedPdfPaths incorrectly treats the closed tab as still open.
  context.subscriptions.push(vscode.window.tabGroups.onDidChangeTabs(forgetClosedPdfTabs));

  void connectAcademicPdfViewerSyncTex();

  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(() => {
    void applyContextLanguageToOpenDocuments();
  }));

  void maybeRunFirstStartSetup(false);

  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration('contextIntellisense.texRootPath') || e.affectsConfiguration('contextIntellisense.xmlPath')) {
      const configuredPath = String(
        vscode.workspace.getConfiguration('contextIntellisense').get('texRootPath', '') || ''
      ).trim();
      loadData(getConfiguredTexRootPath(), getConfiguredLegacyXmlPath());
      refreshDecorations();
    }

    if (e.affectsConfiguration('contextIntellisense.mainFilePath')) {
      refreshDecorations();
    }

  }));

  const triggerArgumentContextCommand = vscode.commands.registerCommand(
    'contextIntellisense.triggerArgumentContext',
    () => triggerArgumentInformation(vscode.window.activeTextEditor)
  );
  context.subscriptions.push(triggerArgumentContextCommand);

  const nextSnippetArgumentCommand = vscode.commands.registerCommand(
    'contextIntellisense.nextSnippetArgument',
    async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      await vscode.commands.executeCommand('jumpToNextSnippetPlaceholder');
      triggerArgumentInformation(editor, true);
    }
  );
  context.subscriptions.push(nextSnippetArgumentCommand);

  const selector = { language: 'context.tex', scheme: 'file' };
  const provider = vscode.languages.registerCompletionItemProvider(
    selector,
    {
      async provideCompletionItems(document, position) {
        if (cache.commandCompletions.length === 0) {
          return [];
        }

        const line = document.lineAt(position.line).text;
        const linePrefix = line.slice(0, position.character);
        const lineSuffix = line.slice(position.character);

        const bracketContext = getDocumentBracketContext(document, position);
        if (bracketContext) {
          const workspaceItems = isWorkspaceArgument(
            bracketContext.commandName,
            bracketContext.argumentIndex,
            bracketContext
          )
            ? await createWorkspaceArgumentItems(bracketContext.commandName)
            : null;
          if (workspaceItems) {
            const segment = getCurrentArgumentSegment(bracketContext).trim().toLowerCase();
            return workspaceItems.filter(item => !segment || item.label.toLowerCase().startsWith(segment));
          }
          const currentSegment = getCurrentArgumentSegment(bracketContext);
          const items = createKeywordAndParameterItems(
            bracketContext.commandName,
            currentSegment,
            bracketContext.argumentIndex,
            bracketContext.bracketCount
          );
          return items;
        }

        const commandMatch = linePrefix.match(/\\([A-Za-z@]*)$/);
        if (commandMatch) {
          return createCommandItems(commandMatch[1]);
        }

        return [];
      }
    },
    '[',
    ',',
    '=',
    ' ',
    '\\'
  );

  context.subscriptions.push(provider);

  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection(event => {
    scheduleArgumentInformation(
      event.textEditor,
      event.kind === vscode.TextEditorSelectionChangeKind.Keyboard
        || event.kind === vscode.TextEditorSelectionChangeKind.Mouse,
      true
    );
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    const editor = vscode.window.activeTextEditor;
    if (editor && editor.document === event.document) {
      scheduleArgumentInformation(editor);
    }
  }));

  const signatureProvider = vscode.languages.registerSignatureHelpProvider(
    selector,
    {
      provideSignatureHelp(document, position) {
        const line = document.lineAt(position.line).text;
        const linePrefix = line.slice(0, position.character);
        const bracketContext = getDocumentBracketContext(document, position);
        if (!bracketContext) {
          return null;
        }

        const commandName = bracketContext.commandName;
        const specs = getCommandSignatureSpecs(commandName, bracketContext.bracketCount)
          .filter(spec => spec.kind !== 'content');
        if (specs.length === 0) {
          return null;
        }

        const activeParameterIndex = Math.min(getActiveParameterIndex(commandName, linePrefix), Math.max(specs.length - 1, 0));
        const activeAssignmentKey = bracketContext && bracketContext.commandName === commandName
          ? getActiveAssignmentKey(bracketContext.currentSegment)
          : '';
        const signatureParts = buildSignatureParts(commandName, specs, {
          activeParameterIndex,
          activeAssignmentKey
        });
        const signature = new vscode.SignatureInformation(signatureParts.label);
        signature.parameters = signatureParts.parameters.map(parameter => new vscode.ParameterInformation(parameter.label, parameter.documentation));

        const help = new vscode.SignatureHelp();
        help.signatures = [signature];
        help.activeSignature = 0;
        help.activeParameter = Math.min(activeParameterIndex, Math.max(signature.parameters.length - 1, 0));
        return help;
      }
    },
    '[',
    '{',
    ',',
    '=',
    ' '
  );

  context.subscriptions.push(signatureProvider);

  const fileDecorationProvider = vscode.window.registerFileDecorationProvider({
    onDidChangeFileDecorations: fileDecorationsEmitter.event,
    provideFileDecoration(uri) {
      const mainFilePath = getResolvedMainFilePath();
      if (!mainFilePath || uri.scheme !== 'file') {
        return undefined;
      }

      if (path.resolve(uri.fsPath) !== path.resolve(mainFilePath)) {
        return undefined;
      }

      return {
        badge: '★',
        tooltip: 'ConTeXt main file',
        color: new vscode.ThemeColor('list.highlightForeground')
      };
    }
  });
  context.subscriptions.push(fileDecorationProvider);
}

function deactivate() {}

module.exports = {
  activate,
  deactivate
};
