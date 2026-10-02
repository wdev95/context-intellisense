/** Creates completion presentation helpers from explicit dependencies. */
module.exports = function createCompletionPresentation(deps) {
  const { vscode, describeArgumentSpec, getDelimiterInfo, normalizeTypeLabel, getCommandSemanticType, getCommandEntry, getArgumentParameterNames, getArgumentParameterValues, getArgumentParameterTypes, uniqueValues } = deps;
  const completionKindBySemanticType = { namespace: vscode.CompletionItemKind.Module, contextStructure: vscode.CompletionItemKind.Class, contextMathEnvironment: vscode.CompletionItemKind.Enum, contextMathCommand: vscode.CompletionItemKind.EnumMember, enumMember: vscode.CompletionItemKind.EnumMember, typeParameter: vscode.CompletionItemKind.TypeParameter, keyword: vscode.CompletionItemKind.Keyword, contextCommand: vscode.CompletionItemKind.Function, contextFallbackCommand: vscode.CompletionItemKind.Function, contextRegister: vscode.CompletionItemKind.Variable, function: vscode.CompletionItemKind.Function };
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
      const parameterNames = getArgumentParameterNames(spec);
      for (const parameterName of parameterNames) {
        if (activeAssignmentKey && specIndex === activeParameterIndex && parameterName !== activeAssignmentKey) {
          continue;
        }

        const values = collectOrderedValues(
          getArgumentParameterValues(spec, entry, parameterName),
          entry?.parameterValueDefaults?.get(parameterName) || new Set()
        );
        const parameterTypes = uniqueValues(
          [...getArgumentParameterTypes(spec, entry, parameterName)]
            .filter(type => /^cd:/i.test(type))
            .map(normalizeTypeLabel)
        );
        const renderedValues = chunkValuesForDisplay([
          ...values.map(item => formatValueCell(item.value, item.isDefault)),
          ...parameterTypes.map(escapeHtmlText)
        ]);

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


/** Maps command metadata to the same semantic type used for its completion icon. */
function getCommandCompletionKind(metadata) {
  const semanticType = getCommandSemanticType(metadata);
  return completionKindBySemanticType[semanticType] || vscode.CompletionItemKind.Function;
}

  return { escapeHtmlText, buildHtmlKeyValueTable, formatValueCell, chunkValuesForDisplay, collectOrderedValues, buildSignatureParts, getCommandCompletionKind };
};
