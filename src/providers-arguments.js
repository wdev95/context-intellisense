const {
  isExternalFigureFile,
  isTemporaryWorkspaceFile,
  workspaceFileWatcherPattern
} = require('./core/workspace-files');
const { isEscaped, maskTeXComments } = require('./core/tex-text');

/** Creates bracket parsing, argument information, and completion services. */
module.exports = function createArgumentProviders(deps) {
  const {
    vscode,
    fs,
    path,
    isContextTexDocument,
    inferredReferenceArguments,
    REFERENCE_TOKEN_PATTERN,
    workspaceReferenceState,
    workspaceFileState,
    isConTeXtTexFilePath,
    getCache,
    getCommandEntry,
    buildSignatureParts,
    getDelimiterInfo,
    makeArgumentPreview,
    uniqueValues
  } = deps;

  const workspaceFileEntries = new Map();
  const workspaceMPLibraries = new Set();
  let workspaceFileRefreshTimer;
  const workspaceFileChangeListeners = [
    vscode.workspace.onDidCreateFiles(invalidateWorkspaceFiles),
    vscode.workspace.onDidDeleteFiles(invalidateWorkspaceFiles),
    vscode.workspace.onDidRenameFiles(invalidateWorkspaceFiles),
    vscode.workspace.onDidChangeWorkspaceFolders(invalidateWorkspaceFiles)
  ];
  const workspaceFileWatcher = vscode.workspace.createFileSystemWatcher(workspaceFileWatcherPattern());
  workspaceFileChangeListeners.push(
    workspaceFileWatcher.onDidCreate(invalidateWorkspaceFiles),
    workspaceFileWatcher.onDidChange(invalidateWorkspaceFiles),
    workspaceFileWatcher.onDidDelete(invalidateWorkspaceFiles),
    workspaceFileWatcher
  );

  /** Invalidates the shared file inventory and all filtered views. */
  function invalidateWorkspaceFiles(event) {
    const events = event?.files || (event?.fsPath ? [event] : []);
    const changedPaths = events.flatMap(file => [file, file.oldUri, file.newUri]
      .filter(uri => uri?.fsPath)
      .map(uri => uri.fsPath));
    if (changedPaths.length > 0 && changedPaths.every(isTemporaryWorkspaceFile)) {
      return;
    }
    workspaceFileState.promise = null;
    workspaceReferenceState.promise = null;
    workspaceMPLibraries.clear();
    workspaceFileEntries.clear();
    clearTimeout(workspaceFileRefreshTimer);
    workspaceFileRefreshTimer = setTimeout(() => {
      workspaceFileRefreshTimer = undefined;
      void initializeWorkspaceFileCache().catch(() => {});
    }, 100);
  }
/** Finds the matching closing delimiter for an opening delimiter. */
function findMatchingDelimiter(text, start, open, close) {
  let depth = 0;
  for (let index = start; index < text.length; index++) {
    if (isEscaped(text, index)) {
      continue;
    }
    if (text[index] === open) {
      depth++;
    } else if (text[index] === close && --depth === 0) {
      return index;
    }
  }
  return text.length;
}

/** Maps top-level command arguments to their immediately preceding command. */
function mapCommandArguments(text) {
  const argumentsByOpen = new Map();
  const commandRe = /\\([A-Za-z]+)/g;
  let commandMatch;
  while ((commandMatch = commandRe.exec(text)) !== null) {
    let position = commandMatch.index + commandMatch[0].length;
    let argumentIndex = 0;
    while (position < text.length) {
      while (/\s/.test(text[position] || '')) {
        position++;
      }
      const open = text[position];
      if (!['[', '{'].includes(open)) {
        break;
      }
      const close = findMatchingDelimiter(text, position, open, open === '[' ? ']' : '}');
      argumentsByOpen.set(position, {
        commandName: commandMatch[1],
        commandIndex: commandMatch.index,
        argumentIndex,
        delimiter: open === '[' ? 'bracket' : 'brace',
        close
      });
      argumentIndex++;
      position = close + 1;
    }
  }
  return argumentsByOpen;
}

/** Returns the square-bracket argument under the cursor. */
function getBracketInvocationContext(linePrefix, lineSuffix = '') {
  const text = linePrefix + lineSuffix;
  const cursor = linePrefix.length;
  const argumentsByOpen = mapCommandArguments(text);
  const stack = [];
  for (let index = 0; index < cursor; index++) {
    if (isEscaped(text, index)) {
      continue;
    }
    if (text[index] === '[' || text[index] === '{') {
      stack.push({ delimiter: text[index], index });
    } else if (text[index] === ']' || text[index] === '}') {
      const expected = text[index] === ']' ? '[' : '{';
      const openIndex = stack.map(item => item.delimiter).lastIndexOf(expected);
      if (openIndex >= 0) {
        stack.splice(openIndex, 1);
      }
    }
  }

  const open = [...stack].reverse().find(item => item.delimiter === '['
    && argumentsByOpen.has(item.index));
  if (!open) {
    return null;
  }

  const argument = argumentsByOpen.get(open.index);
  const nested = stack.filter(item => item.index > open.index);
  const currentSegment = text.slice(open.index + 1, cursor);
  const remainingSegment = text.slice(cursor, argument.close);
  const commandArguments = [...argumentsByOpen.values()]
    .filter(item => item.commandIndex === argument.commandIndex)
    .sort((left, right) => left.argumentIndex - right.argumentIndex)
  const argumentDelimiters = commandArguments.map(item => item.delimiter);

  const context = {
    commandName: argument.commandName,
    argumentIndex: argument.argumentIndex,
    bracketCount: commandArguments.filter(delimiter => delimiter.delimiter === 'bracket').length,
    argumentDelimiters,
    currentSegment,
    remainingSegment,
    isTopLevel: nested.length === 0,
    isEnumListValue: false
  };
  if (nested.length === 1 && nested[0].delimiter === '{') {
    const assignment = getCurrentAssignmentValue(context);
    const spec = assignment && getCommandArgumentSpec(context);
    context.isEnumListValue = spec?.kind === 'assignments'
      && getArgumentParameterValues(spec, getCommandEntry(context.commandName), assignment.key).size > 0;
  }
  return context;
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

/** Returns the final top-level comma-separated segment of an argument. */
function getTopLevelArgumentSegment(text) {
  const segments = getTopLevelArgumentSegments(text);
  return segments[segments.length - 1]?.text || '';
}

/** Splits an argument into comma-separated top-level segments with offsets. */
function getTopLevelArgumentSegments(text) {
  const segments = [];
  let braceDepth = 0;
  let bracketDepth = 0;
  let start = 0;
  for (let index = 0; index < text.length; index += 1) {
    if (isEscaped(text, index)) {
      continue;
    }
    if (text[index] === '{') {
      braceDepth += 1;
    } else if (text[index] === '}') {
      braceDepth = Math.max(0, braceDepth - 1);
    } else if (text[index] === '[') {
      bracketDepth += 1;
    } else if (text[index] === ']') {
      bracketDepth = Math.max(0, bracketDepth - 1);
    } else if (text[index] === ',' && braceDepth === 0 && bracketDepth === 0) {
      segments.push({ text: text.slice(start, index), start });
      start = index + 1;
    }
  }
  segments.push({ text: text.slice(start), start });
  return segments;
}

/** Returns the text of the current comma-separated argument segment. */
function getCurrentArgumentSegment(context) {
  return getTopLevelArgumentSegment(context && context.currentSegment || '');
}

/** Replaces the whole token so insert-mode settings cannot leave its suffix behind. */
function setCompletionRange(items, position, startCharacter, endCharacter = position.character, options = {}) {
  const start = Math.max(0, startCharacter);
  const end = Math.max(position.character, endCharacter);
  for (const item of items) {
    const label = String(typeof item.label === 'string' ? item.label : item.label.label);
    const itemEnd = label.endsWith('=') ? Math.max(end, options.keyEndCharacter ?? end) : end;
    item.range = new vscode.Range(position.line, start, position.line, itemEnd);
    if (options.showAll) {
      const current = options.lineText.slice(start, itemEnd).trim().replace(/=$/, '').toLowerCase();
      const insertText = item.insertText?.value || item.insertText || label;
      const candidate = String(insertText).replace(/\$\{?\d+(?::[^}]*)?\}?/g, '').trim().replace(/=$/, '').toLowerCase();
      item.preselect = !!current && candidate === current;
      item.filterText = `${options.lineText.slice(start, itemEnd)}${item.filterText || label}`;
    }
  }
  return items;
}

/** Returns no completion list when the active context has no suggestions. */
function createDynamicCompletionList(items, isIncomplete = true) {
  return items?.length ? new vscode.CompletionList(items, isIncomplete) : undefined;
}

/** Returns the start of the text currently being completed. */
function getCompletionRangeStart(linePrefix, position, context) {
  const segment = getCurrentArgumentSegment(context);
  const segmentStart = position.character - segment.length;
  const leadingWhitespace = segment.length - segment.trimStart().length;
  if (context.isEnumListValue) {
    return segmentStart + getCurrentAssignmentValue(context).start;
  }
  const equalsIndex = segment.indexOf('=');
  const value = equalsIndex < 0 ? '' : segment.slice(equalsIndex + 1);
  const valueStart = segmentStart + equalsIndex + 1 + value.length - value.trimStart().length;
  const commandStart = linePrefix.lastIndexOf('\\');
  if (commandStart >= valueStart && /^[A-Za-z]*$/.test(linePrefix.slice(commandStart + 1))) {
    return commandStart + 1;
  }
  return equalsIndex >= 0 ? valueStart : segmentStart + leadingWhitespace;
}

/** Finds the end of the token at the cursor, including text to its right. */
function getCompletionRangeEnd(lineSuffix, linePrefix, position, context) {
  const segment = getCurrentArgumentSegment(context);
  const equalsIndex = findAssignmentSeparator(segment);
  const segmentStart = position.character - segment.length;
  const inKey = equalsIndex < 0 || position.character <= segmentStart + equalsIndex;
  const commandStart = linePrefix.lastIndexOf('\\');
  const valueStart = equalsIndex < 0 ? segmentStart : segmentStart + equalsIndex + 1;
  const isCommand = commandStart >= valueStart
    && /^[A-Za-z]*$/.test(linePrefix.slice(commandStart + 1));
  const suffix = lineSuffix;
  let length = 0;
  while (length < suffix.length) {
    const character = suffix[length];
    if (/[\s,\[\]{}]/.test(character) || (inKey && character === '=')) break;
    if (isCommand && !/[A-Za-z]/.test(character)) break;
    length += 1;
  }
  return position.character + length;
}

/** Returns whether a segment is an empty argument or assignment value. */
function isEmptyCompletionSegment(context) {
  if (context.isEnumListValue) {
    return !getCurrentAssignmentValue(context).value;
  }
  const segment = getCurrentArgumentSegment(context).trim();
  if (!segment) {
    return true;
  }
  const equalsIndex = segment.indexOf('=');
  return equalsIndex >= 0 && !segment.slice(equalsIndex + 1).trim()
    && !/^\s*\{/.test(context.remainingSegment || '');
}

/** Resolves the enum member currently being edited in an assignment value. */
function getCurrentAssignmentValue(context) {
  const segment = getCurrentArgumentSegment(context);
  const equalsIndex = findAssignmentSeparator(segment);
  if (equalsIndex < 0) {
    return null;
  }

  const key = segment.slice(0, equalsIndex).trim();
  let start = equalsIndex + 1;
  while (/\s/.test(segment[start] || '')) {
    start += 1;
  }
  let valuePrefix = segment.slice(start);
  const grouped = valuePrefix.startsWith('{');
  if (grouped) {
    valuePrefix = valuePrefix.slice(1);
    start += 1;
  }

  const current = getTopLevelArgumentSegments(valuePrefix).at(-1) || { text: '', start: 0 };
  const leadingSpace = current.text.length - current.text.trimStart().length;
  return {
    key,
    value: current.text.trim(),
    start: start + current.start + leadingSpace,
    grouped
  };
}

/** Opens completion suggestions when the cursor enters an empty argument. */
function triggerEmptyArgumentSuggestions(editor, refresh = false) {
  if (!editor || !isContextTexDocument(editor.document)) {
    return;
  }

  const position = editor.selection.active;
  const argumentContext = getDocumentBracketContext(editor.document, position);
  const argumentSpec = getCommandArgumentSpec(argumentContext);
  const context = argumentContext
    && isEmptyCompletionSegment(argumentContext)
    && argumentSpec
    ? argumentContext
    : null;
  const key = context
    ? `${editor.document.uri.toString()}:${position.line}:${position.character}:${context.commandName}:${context.argumentIndex}:${context.currentSegment}`
    : '';
  if (!context) {
    triggerEmptyArgumentSuggestions.lastPosition = '';
    if (!argumentContext) {
      const line = editor.document.lineAt(editor.selection.active.line).text;
      const prefix = line.slice(0, editor.selection.active.character);
      if (!/\\[A-Za-z]*$/.test(prefix)) {
        void vscode.commands.executeCommand('hideSuggestWidget');
      }
    }
    return;
  }

  if (!refresh && triggerEmptyArgumentSuggestions.lastPosition === key) {
    return;
  }

  triggerEmptyArgumentSuggestions.lastPosition = key;
  void createArgumentCompletionItems(context, true).then(items => {
    if (items.length && vscode.window.activeTextEditor === editor
      && editor.selection.active.isEqual(position)) {
      void vscode.commands.executeCommand('editor.action.triggerSuggest');
    }
  });
}

/** Shows argument information whenever the cursor is inside an argument. */
function triggerArgumentInformation(editor, refresh = false, selectionChange = false) {
  if (!editor || !isContextTexDocument(editor.document)) {
    return;
  }
  if (editor.selections.some(selection => !selection.isEmpty)) {
    void vscode.commands.executeCommand('hideSuggestWidget');
    void vscode.commands.executeCommand('closeParameterHints');
    return;
  }

  const context = getEditorBracketContext(editor);
  if (!context) {
    const line = editor.document.lineAt(editor.selection.active.line).text;
    const prefix = line.slice(0, editor.selection.active.character);
    const commandMatch = prefix.match(/\\([A-Za-z]*)$/);
    if (commandMatch) {
      if (selectionChange) {
        return;
      }
      if (createCommandItems(commandMatch[1], editor.document.languageId === 'context-internal-tex').length) {
        void vscode.commands.executeCommand('editor.action.triggerSuggest');
      }
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
  setTimeout(() => triggerArgumentInformation(editor, refresh, selectionChange), 0);
}

/** Resolves the active argument schema shared by editor features and diagnostics. */
function getCommandArgumentSpec(context) {
  if (!context) return null;
  const specs = getCommandSignatureSpecs(
    context.commandName,
    context.bracketCount,
    context.argumentDelimiters,
    context.argumentIndex,
    context.currentSegment,
    context.remainingSegment
  );
  return specs?.[context.argumentIndex] || null;
}

/** Returns whether a signature still requires an argument after its brackets. */
function hasRequiredArgumentAfterBrackets(specs, bracketCount) {
  let seenBrackets = 0;
  const argumentsList = specs || [];
  for (let index = 0; index < argumentsList.length; index += 1) {
    if (argumentsList[index].delimiter === 'bracket' && ++seenBrackets >= bracketCount) {
      return argumentsList.slice(index + 1).some(argument => !argument.optional);
    }
  }
  return false;
}

/** Selects the XML signature that matches the entered delimiters and position. */
/** Returns the comma-separated segment surrounding the cursor. */
function getActiveSegmentAtCursor(currentSegment, remainingSegment) {
  if (remainingSegment === undefined) {
    return String(currentSegment || '').trim();
  }
  const text = `${currentSegment || ''}${remainingSegment || ''}`;
  const cursor = String(currentSegment || '').length;
  return getTopLevelArgumentSegments(text).find(segment =>
    cursor >= segment.start && cursor <= segment.start + segment.text.length
  )?.text.trim() || '';
}

/** Returns the argument schema matching the source and the cursor's active field. */
function getCommandSignatureSpecs(commandName, bracketCount = 0, argumentDelimiters = [], activeArgumentIndex = -1, currentSegment = '', remainingSegment = '') {
  if (bracketCount > 0) {
    const variants = getCache().commandArgumentSpecVariants.get(commandName) || [];
    if (variants.length === 0) {
      return getCache().commandArgumentSpecs.get(commandName) || [];
    }
    const matching = variants.filter(specs =>
      specs.filter(spec => spec.delimiter === 'bracket').length === bracketCount
    );
    if (matching.length === 0) {
      return [];
    }
    const delimiterMatching = argumentDelimiters.length > 0
      ? matching.filter(specs => argumentDelimiters.every((delimiter, index) =>
        specs[index] && specs[index].delimiter === delimiter
      ))
      : matching;
    if (delimiterMatching.length === 0) {
      return [];
    }
    let candidates = delimiterMatching;
    const complete = candidates.filter(specs => !hasRequiredArgumentAfterBrackets(specs, bracketCount));
    if (complete.length > 0) {
      candidates = complete;
    }
    const activeSegment = getActiveSegmentAtCursor(currentSegment, remainingSegment);
    const previousSegment = activeSegment || String(currentSegment || '')
      .replace(/,\s*$/, '')
      .split(',')
      .at(-1)
      .trim();
    if (activeArgumentIndex >= 0 && candidates.length > 1) {
      const activeSpecs = candidates.map(specs => specs[activeArgumentIndex]);
      const assignmentNames = activeSpecs
        .filter(spec => spec?.kind === 'assignments')
        .flatMap(spec => getArgumentParameterNames(spec));
      const fieldName = previousSegment.split('=', 1)[0].trim().toLowerCase();
      const isAssignmentField = previousSegment.includes('=')
        || (fieldName && assignmentNames.some(name => name.toLowerCase().startsWith(fieldName)));
      const preferredKind = isAssignmentField ? 'assignments' : 'keywords';
      let kindMatching = candidates.filter(specs =>
        specs[activeArgumentIndex]?.kind === preferredKind
      );
      if (kindMatching.length > 0) {
        candidates = kindMatching;
      }
      if (preferredKind === 'assignments') {
        const exactKeyMatches = candidates.filter(specs =>
          getArgumentParameterNames(specs[activeArgumentIndex]).some(name =>
            name.toLowerCase() === fieldName
          )
        );
        const prefixKeyMatches = candidates.filter(specs =>
          getArgumentParameterNames(specs[activeArgumentIndex]).some(name =>
            name.toLowerCase().startsWith(fieldName)
          )
        );
        if (exactKeyMatches.length > 0) {
          candidates = exactKeyMatches;
        } else if (prefixKeyMatches.length > 0) {
          candidates = prefixKeyMatches;
        } else {
          const constrained = candidates.filter(specs => {
            const spec = specs[activeArgumentIndex];
            return !spec?.allowsArbitraryKeys && getArgumentParameterNames(spec).length > 0;
          });
          if (constrained.length > 0) candidates = constrained;
        }
      }
    }
    candidates.sort((left, right) =>
      (left[activeArgumentIndex]?.argumentPosition ?? activeArgumentIndex)
      - (right[activeArgumentIndex]?.argumentPosition ?? activeArgumentIndex)
    );
    // Do not show optional trailing arguments that are absent from the source.
    if (candidates.length > 1) {
      const shortestSignatureLength = Math.min(...candidates.map(specs => specs.length));
      candidates = candidates.filter(specs => specs.length === shortestSignatureLength);
    }
    if (candidates.length > 0) {
      return candidates[0];
    }
  }
  return getCache().commandArgumentSpecs.get(commandName) || [];
}

/** Returns whether an XML parameter is a concrete assignment key. */
function isConcreteParameterName(name) {
  return /^[a-z][a-z0-9]*(?:[-_][a-z0-9]+)*$/i.test(String(name || '').trim());
}

/** Returns direct and inherited concrete keys available in an assignment argument. */
function getArgumentParameterNames(argumentSpec) {
  const names = [
    ...(argumentSpec?.parameterNames || []),
    ...(argumentSpec?.parameterSources || []).flatMap(sourceName =>
      [...(getCommandEntry(sourceName)?.parameters || [])]
    )
  ].map(name => String(name).trim());
  return [...new Map(names.filter(isConcreteParameterName).map(name => [name.toLowerCase(), name])).values()];
}

/** Returns XML-defined values from the active argument and inherited sources. */
function getArgumentParameterValues(argumentSpec, entry, parameterName) {
  const key = String(parameterName || '').trim().toLowerCase();
  const values = new Set(argumentSpec?.parameterValues?.get(key) || []);
  for (const value of entry?.parameterValues?.get(key) || []) {
    values.add(value);
  }
  for (const sourceName of argumentSpec?.parameterSources || []) {
    for (const value of getCommandEntry(sourceName)?.parameterValues?.get(key) || []) {
      values.add(value);
    }
  }
  return values;
}

/** Returns XML-declared types for an assignment parameter. */
function getArgumentParameterTypes(argumentSpec, entry, parameterName) {
  const key = String(parameterName || '').trim().toLowerCase();
  const types = new Set(argumentSpec?.parameterTypes?.get(key) || []);
  for (const type of entry?.parameterTypes?.get(key) || []) {
    types.add(type);
  }
  return types;
}

/** Returns whether an XML argument type identifies a ConTeXt reference. */
function isReferenceType(type) {
  return String(type || '').trim().toLowerCase() === 'cd:reference';
}

/** Returns whether an XML argument type identifies a workspace file. */
function isFileType(type) {
  return String(type || '').trim().toLowerCase() === 'cd:file';
}

/** Determines whether this argument defines a reference instead of consuming one. */
function isReferenceDefinitionContext(context, spec = getCommandArgumentSpec(context)) {
  if (!context) {
    return false;
  }
  const metadata = getCache().commandMetadata.get(context.commandName);
  const pairedEnvironment = getCache().commandMetadata.get(`start${context.commandName}`);
  if (metadata?.referenceSourceArgument === context.argumentIndex) {
    return true;
  }
  if (spec?.kind === 'keywords'
    && (String(metadata?.type).toLowerCase() === 'environment'
      || String(pairedEnvironment?.type).toLowerCase() === 'environment')
    && (spec.keywordTypes || []).some(isReferenceType)) {
    return true;
  }
  if (!spec && inferredReferenceArguments.has(`${context.commandName}:${context.argumentIndex}`)) {
    return true;
  }
  if (spec?.kind === 'assignments'
    && /^reference$/i.test(getActiveAssignmentKey(context.currentSegment))) {
    return true;
  }
  return false;
}

const WORKSPACE_FILE_ARGUMENT_POLICIES = {
  externalfigure: {
    argumentIndex: 0,
    matches: filePath => isExternalFigureFile(filePath),
    validate: true,
    additionalValues: () => workspaceMPLibraries.has('dum') ? ['dummy'] : [],
    detail: 'Workspace image'
  },
  environment: {
    argumentIndex: 0,
    matches: filePath => path.extname(filePath).toLowerCase() === '.tex',
    validate: true,
    stripTexExtension: true,
    detail: 'ConTeXt environment'
  },
  component: {
    argumentIndex: 0,
    matches: filePath => path.extname(filePath).toLowerCase() === '.tex',
    validate: true,
    stripTexExtension: true,
    detail: 'ConTeXt component'
  }
};
const GENERIC_WORKSPACE_FILE_ARGUMENT = {
  matches: () => true,
  validate: false,
  stripTexExtension: false,
  detail: 'Workspace file'
};

/** Returns the cached workspace file inventory, scanning it only once per change. */
async function getWorkspaceFiles() {
  if (!workspaceFileState.promise) {
    workspaceFileState.promise = vscode.workspace.findFiles('**/*', '**/{.git,node_modules}/**')
      .catch(error => {
        workspaceFileState.promise = null;
        throw error;
      });
  }
  return workspaceFileState.promise;
}

/** Preloads the workspace file inventory before the first completion request. */
async function initializeWorkspaceFileCache() {
  await Promise.all(Object.values(WORKSPACE_FILE_ARGUMENT_POLICIES).map(getWorkspaceFileEntries));
}

/** Resolves a shared workspace file view from the argument's XML file type. */
function getWorkspaceFileArgument(context, spec) {
  if (spec?.kind !== 'keywords' || !(spec.keywordTypes || []).some(isFileType)) return null;
  const policy = WORKSPACE_FILE_ARGUMENT_POLICIES[String(context.commandName).toLowerCase()] || {};
  if (policy.argumentIndex !== undefined && policy.argumentIndex !== context.argumentIndex) return null;
  return Object.keys(policy).length > 0 ? policy : GENERIC_WORKSPACE_FILE_ARGUMENT;
}

/** Returns matching files with separate display and insertion names. */
async function getWorkspaceFileEntries(rule) {
  if (!workspaceFileEntries.has(rule)) {
    const pending = getWorkspaceFiles()
      .then(files => files
        .filter(uri => !isTemporaryWorkspaceFile(uri.fsPath) && rule.matches(uri.fsPath, path))
        .sort((left, right) => left.fsPath.localeCompare(right.fsPath))
        .map(uri => {
          const relativePath = vscode.workspace.asRelativePath(uri, false).replace(/\\/g, '/');
          let value = path.posix.basename(relativePath);
          if (rule.stripTexExtension) {
            value = value.replace(/\.tex$/i, '');
          }
          return { uri, relativePath, value, label: path.posix.basename(relativePath) };
        }))
      .catch(error => {
        workspaceFileEntries.delete(rule);
        throw error;
      });
    workspaceFileEntries.set(rule, pending);
  }
  const entries = await workspaceFileEntries.get(rule);
  return [
    ...entries,
    ...(rule.additionalValues?.() || []).map(value => ({ value, label: value, relativePath: '', virtual: true }))
  ];
}

/** Creates filtered file completions from the shared workspace inventory. */
async function createWorkspaceFileItems(rule) {
  if (rule.additionalValues) await scanWorkspaceReferences();
  const files = await getWorkspaceFileEntries(rule);
  return files.map(({ relativePath, value, label }) => {
    const item = new vscode.CompletionItem(label, vscode.CompletionItemKind.File);
    item.insertText = value;
    item.filterText = `${value} ${label}`;
    const directory = path.posix.dirname(relativePath);
    item.detail = directory !== '.' ? `${rule.detail} · ${directory}` : rule.detail;
    return item;
  });
}

/** Finds an assignment separator outside nested TeX delimiters. */
function findAssignmentSeparator(segment) {
  let braceDepth = 0;
  let bracketDepth = 0;
  for (let index = 0; index < segment.length; index += 1) {
    if (isEscaped(segment, index)) {
      continue;
    }
    if (segment[index] === '{') {
      braceDepth += 1;
    } else if (segment[index] === '}') {
      braceDepth = Math.max(0, braceDepth - 1);
    } else if (segment[index] === '[') {
      bracketDepth += 1;
    } else if (segment[index] === ']') {
      bracketDepth = Math.max(0, bracketDepth - 1);
    } else if (segment[index] === '=' && braceDepth === 0 && bracketDepth === 0) {
      return index;
    }
  }
  return -1;
}

/** Returns a literal argument value, excluding one enclosing brace pair. */
function normalizeArgumentValue(rawValue) {
  let value = String(rawValue || '').trim();
  if (value.startsWith('{') && findMatchingDelimiter(value, 0, '{', '}') === value.length - 1) {
    value = value.slice(1, -1).trim();
  }
  return value;
}

/** Returns the exact source span and normalized value of an argument segment. */
function getArgumentValueSpan(rawValue, sourceStart) {
  const text = String(rawValue || '');
  const leadingSpace = text.length - text.trimStart().length;
  const trailingSpace = text.length - text.trimEnd().length;
  let start = sourceStart + leadingSpace;
  let value = text.slice(leadingSpace, text.length - trailingSpace);
  let grouped = false;
  if (value.startsWith('{') && findMatchingDelimiter(value, 0, '{', '}') === value.length - 1) {
    grouped = true;
    start += 1;
    value = value.slice(1, -1);
  }
  const innerLeadingSpace = value.length - value.trimStart().length;
  const innerTrailingSpace = value.length - value.trimEnd().length;
  start += innerLeadingSpace;
  value = value.slice(innerLeadingSpace, value.length - innerTrailingSpace);
  return {
    value,
    start,
    end: start + value.length,
    grouped
  };
}

/** Returns whether a value is dynamic and cannot be checked statically. */
function isDynamicArgumentValue(value) {
  return !value || /\\[A-Za-z@]/.test(value) || /#\d*/.test(value);
}

/** Maps a text offset to a VS Code position without opening the document. */
function positionAtOffset(offset, lineStarts) {
  let low = 0;
  let high = lineStarts.length;
  while (low + 1 < high) {
    const middle = (low + high) >> 1;
    if (lineStarts[middle] <= offset) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return new vscode.Position(low, offset - lineStarts[low]);
}

/** Returns whether a filename matches a file from its argument-specific list. */
function isWorkspaceFileValue(value, rule, entries) {
  const normalized = normalizeArgumentValue(value);
  if (!normalized || /^(?:https?|data):/i.test(normalized) || isDynamicArgumentValue(normalized)) {
    return true;
  }
  const basename = path.posix.basename(normalized.replace(/\\/g, '/')).toLowerCase();
  const candidate = rule.stripTexExtension ? basename.replace(/\.tex$/i, '') : basename;
  return entries.some(entry => {
    const listed = entry.value.toLowerCase();
    return candidate === listed || (!path.posix.extname(candidate) && candidate === path.posix.parse(listed).name.toLowerCase());
  });
}

/** Builds warnings for invalid values, missing files, and unresolved references. */
async function getArgumentDiagnostics(sourceText, definedReferences = new Set()) {
  const text = maskTeXComments(String(sourceText || ''));
  const lineStarts = [0];
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === '\n') {
      lineStarts.push(index + 1);
    }
  }

  const commandArguments = new Map();
  for (const [open, argument] of mapCommandArguments(text)) {
    const argumentsForCommand = commandArguments.get(argument.commandIndex) || [];
    argumentsForCommand.push({ open, ...argument });
    commandArguments.set(argument.commandIndex, argumentsForCommand);
  }

  const diagnostics = [];
  const report = (start, end, message, code) => {
    const range = new vscode.Range(
      positionAtOffset(start, lineStarts),
      positionAtOffset(end, lineStarts)
    );
    const diagnostic = new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Warning);
    diagnostic.source = 'ConTeXt IntelliSense';
    diagnostic.code = code;
    diagnostics.push(diagnostic);
  };
  /** Reports literal reference values that have no workspace definition. */
  const reportUndefinedReferences = (rawValue, sourceStart, definedReferences) => {
    const valueSpan = getArgumentValueSpan(rawValue, sourceStart);
    for (const member of getTopLevelArgumentSegments(valueSpan.value)) {
      const span = getArgumentValueSpan(member.text, valueSpan.start + member.start);
      const value = span.value;
      if (!value || isDynamicArgumentValue(value)
        || !/^[A-Za-z0-9][A-Za-z0-9:._-]*$/.test(value)
        || definedReferences.has(value)) {
        continue;
      }
      report(span.start, span.start + value.length,
        `Reference "${value}" is not defined in the workspace.`, 'undefined-reference');
    }
  };

  for (const argumentsForCommand of commandArguments.values()) {
    argumentsForCommand.sort((left, right) => left.argumentIndex - right.argumentIndex);
    const first = argumentsForCommand[0];
    const delimiters = argumentsForCommand.map(argument => argument.delimiter);
    const bracketCount = delimiters.filter(delimiter => delimiter === 'bracket').length;
    for (const argument of argumentsForCommand) {
      if (argument.delimiter !== 'bracket') {
        continue;
      }
      const contentStart = argument.open + 1;
      const content = text.slice(contentStart, argument.close);
      const context = {
        commandName: first.commandName,
        argumentIndex: argument.argumentIndex,
        bracketCount,
        argumentDelimiters: delimiters,
        currentSegment: content,
        remainingSegment: ''
      };
      const spec = getCommandArgumentSpec(context);
      if (!spec) {
        continue;
      }

      const fileRule = getWorkspaceFileArgument(context, spec);
      if (fileRule?.validate
        && (fileRule.argumentIndex === undefined || fileRule.argumentIndex === context.argumentIndex)) {
        const entries = await getWorkspaceFileEntries(fileRule);
        const segments = spec.list ? getTopLevelArgumentSegments(content) : [{ text: content, start: 0 }];
        for (const segment of segments) {
          const span = getArgumentValueSpan(segment.text, contentStart + segment.start);
          const value = span.value;
          if (!value || isWorkspaceFileValue(value, fileRule, entries)) {
            continue;
          }
          report(span.start, span.end,
            `File "${value}" is not in the workspace list for \\${context.commandName}.`, 'file-not-in-workspace');
        }
      }

      const commandCategory = getCache().commandMetadata.get(context.commandName)?.category;
      if (commandCategory !== 'bibliography'
        && spec.kind === 'keywords'
        && (spec.keywordTypes || []).some(isReferenceType)
        && !isReferenceDefinitionContext(context, spec)) {
        for (const segment of getTopLevelArgumentSegments(content)) {
          reportUndefinedReferences(segment.text, contentStart + segment.start, definedReferences);
        }
      }

      if (spec.kind === 'keywords' && spec.keywordValues?.length
        && !spec.allowsArbitraryValues
        && !(spec.keywordTypes || []).some(type => isReferenceType(type) || isFileType(type))) {
        const argumentSpan = getArgumentValueSpan(content, contentStart);
        const values = spec.list || argumentSpan.grouped
          ? getTopLevelArgumentSegments(argumentSpan.value)
          : [{ text: argumentSpan.value, start: 0 }];
        const allowedValues = new Set(spec.keywordValues.map(value => value.toLowerCase()));
        for (const segment of values) {
          const valueSpan = getArgumentValueSpan(segment.text, argumentSpan.start + segment.start);
          const value = valueSpan.value;
          if (value && !isDynamicArgumentValue(value) && !allowedValues.has(value.toLowerCase())) {
            report(valueSpan.start, valueSpan.end,
              `Invalid value "${value}"; expected: ${[...spec.keywordValues].join(', ')}.`, 'invalid-keyword-value');
          }
        }
      }

      if (spec.kind !== 'assignments') {
        continue;
      }
      const entry = getCommandEntry(context.commandName);
      for (const segment of getTopLevelArgumentSegments(content)) {
        const segmentContext = { ...context, currentSegment: segment.text };
        const segmentSpec = getCommandArgumentSpec(segmentContext);
        if (segmentSpec?.kind !== 'assignments') continue;
        const separator = findAssignmentSeparator(segment.text);
        if (separator < 0) {
          continue;
        }
        const key = segment.text.slice(0, separator).trim();
        const allowedKeys = getArgumentParameterNames(segmentSpec);
        if (allowedKeys.length && !segmentSpec.allowsArbitraryKeys
          && !allowedKeys.some(name => name.toLowerCase() === key.toLowerCase())) {
          const expected = allowedKeys.join(', ');
          const keySpan = getArgumentValueSpan(segment.text.slice(0, separator), contentStart + segment.start);
          report(keySpan.start, keySpan.end,
            `Invalid key "${key}"; expected: ${expected}.`, 'invalid-argument-key');
          continue;
        }
        const allowedValues = getArgumentParameterValues(segmentSpec, entry, key);
        const allowedTypes = getArgumentParameterTypes(segmentSpec, entry, key);
        if (commandCategory !== 'bibliography' && !/^reference$/i.test(key)
          && [...allowedTypes].some(isReferenceType)) {
          reportUndefinedReferences(
            segment.text.slice(separator + 1),
            contentStart + segment.start + separator + 1,
            definedReferences
          );
        }
        if (allowedValues.size === 0 || [...allowedTypes].some(type => /^cd:/i.test(type))) {
          continue;
        }
        const rawValue = segment.text.slice(separator + 1);
        const span = getArgumentValueSpan(rawValue, contentStart + segment.start + separator + 1);
        const expected = [...allowedValues];
        const allowed = expected.slice(0, 8).join(', ');
        const suffix = expected.length > 8 ? ', …' : '';
        const values = span.grouped
          ? getTopLevelArgumentSegments(span.value)
          : [{ text: span.value, start: 0 }];
        for (const valuePart of values) {
          const valueSpan = getArgumentValueSpan(valuePart.text, span.start + valuePart.start);
          const value = valueSpan.value;
          if (!value || isDynamicArgumentValue(value)
            || [...allowedValues].some(item => item.toLowerCase() === value.toLowerCase())) {
            continue;
          }
          report(valueSpan.start, valueSpan.end,
            `Invalid value "${value}" for ${key}; expected: ${allowed}${suffix}.`, 'invalid-enum-value');
        }
      }
    }
  }

  return diagnostics;
}

/** Scans workspace ConTeXt files, preferring unsaved text from open documents. */
async function scanWorkspaceArgumentDiagnostics() {
  const definedReferences = await scanWorkspaceReferences();
  const files = (await getWorkspaceFiles()).filter(file => isConTeXtTexFilePath(file.fsPath));
  const openDocuments = new Map(vscode.workspace.textDocuments.map(document => [document.uri.toString(), document]));
  const results = [];
  for (let offset = 0; offset < files.length; offset += 32) {
    const batch = await Promise.all(files.slice(offset, offset + 32).map(async uri => {
      const document = openDocuments.get(uri.toString());
      try {
        const sourceText = document
          ? document.getText()
          : await fs.promises.readFile(uri.fsPath, 'utf8');
        return { uri, diagnostics: await getArgumentDiagnostics(sourceText, definedReferences) };
      } catch {
        return { uri, diagnostics: [] };
      }
    }));
    results.push(...batch);
  }
  return results;
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

/** Extracts reference definitions from assignments, XML-typed environments, and legacy macros. */
function parseContextReferences(content) {
  const references = new Set();
  const commandPattern = /\\(?:definepage|definereference|label|pagereference|reference|setpagereference|setreference|textreference)\s*(?:\{([^{}]*)\}|\[([^\]]*)\])/gi;
  inferReferenceArguments(content);
  const addValues = value => {
    for (const candidate of String(value || '').split(',')) {
      const reference = candidate.trim();
      if (/^[A-Za-z0-9][A-Za-z0-9:._-]*$/.test(reference)) {
        references.add(reference);
      }
    }
  };

  const mappedArguments = [...mapCommandArguments(content)];
  const argumentsByCommand = new Map();
  for (const [, argument] of mappedArguments) {
    const siblings = argumentsByCommand.get(argument.commandIndex) || [];
    siblings.push(argument);
    argumentsByCommand.set(argument.commandIndex, siblings);
  }
  for (const [open, argument] of mappedArguments) {
    if (argument.delimiter !== 'bracket') {
      continue;
    }
    const argumentText = content.slice(open + 1, argument.close);
    const commandArguments = argumentsByCommand.get(argument.commandIndex)
      .sort((left, right) => left.argumentIndex - right.argumentIndex);
    const bracketCount = commandArguments.filter(item => item.delimiter === 'bracket').length;
    const argumentDelimiters = commandArguments.map(item => item.delimiter);
    const context = {
      commandName: argument.commandName,
      argumentIndex: argument.argumentIndex,
      bracketCount,
      argumentDelimiters,
      currentSegment: argumentText
    };
    const spec = getCommandArgumentSpec({
      commandName: argument.commandName,
      argumentIndex: argument.argumentIndex,
      bracketCount,
      argumentDelimiters,
      currentSegment: argumentText,
      remainingSegment: ''
    });
    if (getCache().commandMetadata.get(argument.commandName)?.category === 'bibliography') {
      continue;
    }
    for (const segment of getTopLevelArgumentSegments(argumentText)) {
      const separator = findAssignmentSeparator(segment.text);
      if (separator >= 0 && /^reference$/i.test(segment.text.slice(0, separator).trim())) {
        const value = getArgumentValueSpan(segment.text.slice(separator + 1), 0).value;
        addValues(value);
      }
    }
    if (isReferenceDefinitionContext(context, spec)) {
      for (const segment of getTopLevelArgumentSegments(argumentText)) {
        const value = getArgumentValueSpan(segment.text, 0).value;
        if (findAssignmentSeparator(value) < 0) {
          addValues(value);
        }
      }
    }
  }
  let match;
  while ((match = commandPattern.exec(content)) !== null) {
    addValues(match[1] || match[2]);
  }
  return references;
}

/** Infers reference-bearing argument positions from common ConTeXt usage. */
/** Infers reference-typed argument positions without treating uses as definitions. */
function inferReferenceArguments(content) {
  const commandPattern = /\\([A-Za-z]+)(?=\s*[\[{])/g;
  let commandMatch;
  while ((commandMatch = commandPattern.exec(content)) !== null) {
    let position = commandMatch.index + commandMatch[0].length;
    let argumentIndex = 0;
    const argumentDelimiters = [];
    while (position < content.length) {
      while (/\s/.test(content[position] || '')) {
        position += 1;
      }
      const open = content[position];
      if (open !== '[' && open !== '{') {
        break;
      }
      const close = findMatchingDelimiter(content, position, open, open === '[' ? ']' : '}');
      const argument = content.slice(position + 1, close).trim();
      argumentDelimiters.push(open === '[' ? 'bracket' : 'brace');
      const compact = argument.replace(/[{}\s]/g, '');
      const spec = getCommandArgumentSpec({
        commandName: commandMatch[1],
        argumentIndex,
        bracketCount: argumentDelimiters.filter(delimiter => delimiter === 'bracket').length,
        argumentDelimiters,
        currentSegment: argument,
        remainingSegment: ''
      });
      const parameterName = spec?.kind === 'assignments'
        ? getActiveAssignmentKey(argument)
        : '';
      const typedReference = spec && (
        spec.kind === 'keywords'
          ? (spec.keywordTypes || []).some(isReferenceType)
          : spec.kind === 'assignments'
            && [...getArgumentParameterTypes(spec, getCommandEntry(commandMatch[1]), parameterName)]
              .some(isReferenceType)
      );
      const values = (parameterName ? argument.slice(argument.indexOf('=') + 1) : compact)
        .split(',')
        .filter(Boolean);
      const genericReference = !spec && values.length > 0;
      const referenceValues = (typedReference || genericReference) && values.every(value => {
        REFERENCE_TOKEN_PATTERN.lastIndex = 0;
        return REFERENCE_TOKEN_PATTERN.test(value);
      });
      if (referenceValues) {
        inferredReferenceArguments.add(`${commandMatch[1]}:${argumentIndex}`);
      }
      REFERENCE_TOKEN_PATTERN.lastIndex = 0;
      argumentIndex += 1;
      position = close < content.length ? close + 1 : content.length;
    }
  }
}

/** Scans workspace sources for references and inferred reference arguments. */
async function scanWorkspaceReferences() {
  if (workspaceReferenceState.promise) {
    return workspaceReferenceState.promise;
  }
  workspaceReferenceState.promise = (async () => {
    const files = await vscode.workspace.findFiles(
      '**/*.{tex,mkiv,mkvi,mkii,mkix,mkxl,mklx,mkxi,cld,lmt}',
      '**/{.git,node_modules}/**'
    );
    const references = new Set();
    workspaceMPLibraries.clear();
    const openDocuments = new Map(vscode.workspace.textDocuments.map(document => [document.uri.toString(), document]));
    for (const uri of files) {
      try {
        const content = maskTeXComments(openDocuments.get(uri.toString())?.getText()
          ?? await fs.promises.readFile(uri.fsPath, 'utf8'));
        for (const reference of parseContextReferences(content)) {
          references.add(reference);
        }
        for (const [open, argument] of mapCommandArguments(content)) {
          if (argument.commandName.toLowerCase() === 'usemplibrary' && argument.argumentIndex === 0) {
            for (const member of getTopLevelArgumentSegments(content.slice(open + 1, argument.close))) {
              const library = normalizeArgumentValue(member.text).toLowerCase();
              if (library) workspaceMPLibraries.add(library);
            }
          }
        }
      } catch {
        // Ignore files that disappear or cannot be read while scanning.
      }
    }
    return references;
  })();
  return workspaceReferenceState.promise;
}

/** Returns ConTeXt references from every supported workspace source file. */
async function createReferenceItems() {
  const references = await scanWorkspaceReferences();
  return Array.from(references).sort((left, right) => left.localeCompare(right)).map(reference => {
    const item = new vscode.CompletionItem(reference, vscode.CompletionItemKind.Reference);
    item.insertText = reference;
    item.filterText = reference;
    item.detail = 'ConTeXt reference';
    return item;
  });
}

/** Returns whether the active XML argument accepts a ConTeXt reference. */
function isReferenceArgumentContext(context, spec) {
  if (!context || !spec || isReferenceDefinitionContext(context, spec)) {
    return false;
  }
  const entry = getCommandEntry(context.commandName);
  if (spec.kind === 'keywords') {
    return (spec.keywordTypes || []).some(isReferenceType);
  }
  if (spec.kind === 'assignments') {
    const parameterName = getActiveAssignmentKey(context.currentSegment);
    return [...getArgumentParameterTypes(spec, entry, parameterName)].some(isReferenceType);
  }
  return false;
}

/** Returns completion items for special workspace-backed bracket arguments. */
async function createWorkspaceArgumentItems(context, spec) {
  if (isReferenceDefinitionContext(context, spec)) {
    return [];
  }
  const fileArgument = getWorkspaceFileArgument(context, spec);
  if (fileArgument) {
    return createWorkspaceFileItems(fileArgument);
  }
  if (context.commandName === 'cite' && context.argumentIndex === context.bracketCount - 1) {
    return createCitationItems();
  }
  if (isReferenceArgumentContext(context, spec)) {
    return createReferenceItems();
  }
  return null;
}

function getActiveAssignmentKey(currentSegment, remainingSegment = '') {
  const rawSegment = remainingSegment
    ? getActiveSegmentAtCursor(currentSegment, remainingSegment)
    : currentSegment || '';
  const segment = getTopLevelArgumentSegment(rawSegment);
  const eqIndex = segment.indexOf('=');
  if (eqIndex < 0) {
    return '';
  }

  return segment.slice(0, eqIndex).trim().toLowerCase();
}

/** Returns the stable order key for a command's argument form. */
function getCommandVariantSortKey(argumentSpecs) {
  const specs = (argumentSpecs || []).filter(spec => spec.kind !== 'content');
  if (specs.length === 0) {
    return '0_00';
  }

  const bracketCount = specs.filter(spec => spec.delimiter === 'bracket').length;
  if (bracketCount > 0) {
    return `1_${String(bracketCount).padStart(2, '0')}_${String(specs.length).padStart(2, '0')}`;
  }
  if (specs.some(spec => spec.delimiter === 'brace')) {
    return `2_${String(specs.length).padStart(2, '0')}`;
  }
  return '3_00';
}

/** Returns whether a command name uses the public external notation. */
function isPublicCommandName(commandName) {
  return /^[A-Za-z][A-Za-z0-9]*$/.test(String(commandName || ''));
}

function createCommandItems(prefixPart, includeInternal = false) {
  const lower = (prefixPart || '').toLowerCase();
  const items = [];

  for (const completion of getCache().commandCompletions) {
    const label = completion.label;
    const compareText = label.startsWith('\\') ? label.slice(1) : label;
    if (!includeInternal && !isPublicCommandName(compareText)) {
      continue;
    }
    if (!compareText.toLowerCase().startsWith(lower)) {
      continue;
    }

    const argumentSpecs = completion.argumentSpecs || getCommandSignatureSpecs(compareText);
    const item = new vscode.CompletionItem(makeArgumentPreview(compareText, argumentSpecs), completion.kind);
    item.insertText = completion.insertText;
    const exactMatch = compareText.toLowerCase() === lower ? '0' : '1';
    const casePriority = compareText === compareText.toLowerCase() ? '0' : '1';
    const sourcePriority = String(completion.sourcePriority || 0).padStart(2, '0');
    const commandLength = String(compareText.length).padStart(4, '0');
    item.sortText = `${exactMatch}_${commandLength}_${casePriority}_${sourcePriority}_${getCommandVariantSortKey(argumentSpecs)}_${completion.sortWeight || '50'}_${compareText.toLowerCase()}_${completion.variantIndex || 0}`;
    item.filterText = compareText.toLowerCase();
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

function createValueItems(values, filter = '') {
  const normalizedFilter = filter.trim().toLowerCase();
  return [...new Set(values)]
    .filter(value => !normalizedFilter || value.toLowerCase().startsWith(normalizedFilter))
    .map((value, index) => {
      const item = new vscode.CompletionItem(value, vscode.CompletionItemKind.EnumMember);
      item.insertText = value;
      item.sortText = String(index).padStart(6, '0');
      item.filterText = value;
      return item;
    });
}

function createKeywordAndParameterItems(context, argumentSpec, showAll = false) {
  if (!context) {
    return [];
  }

  const { commandName } = context;
  const currentSegment = getCurrentArgumentSegment(context);
  const entry = getCommandEntry(commandName);

  if (argumentSpec && argumentSpec.kind === 'keywords') {
    const segment = (currentSegment || '').trim();
    if (segment.startsWith('\\')) {
      return createCommandItems(showAll ? '' : segment.slice(1));
    }

    return createValueItems(argumentSpec.keywordValues || [], showAll ? '' : segment);
  }

  if (argumentSpec && argumentSpec.kind !== 'assignments') {
    return [];
  }

  function createAssignmentItems(filterSegment = '') {
    const out = [];

    const parameterNames = getArgumentParameterNames(argumentSpec);
    for (const parameter of parameterNames) {
      const label = `${parameter}=`;
      const item = new vscode.CompletionItem(label, vscode.CompletionItemKind.Enum);
      item.insertText = new vscode.SnippetString(`${parameter}=$0`);
      item.sortText = `1_${label}`;
      item.filterText = label;
      out.push(item);
    }

    const normalizedFilter = String(filterSegment || '').trim().toLowerCase();
    return normalizedFilter && !showAll
      ? out.filter(item => item.label.toString().toLowerCase().startsWith(normalizedFilter))
      : out;
  }

  const rawSegment = currentSegment;
  const segment = rawSegment.trim();
  if (rawSegment.includes('=')) {
    const eqIndex = rawSegment.indexOf('=');
    const enumValue = context.isEnumListValue ? getCurrentAssignmentValue(context) : null;
    const key = enumValue?.key || rawSegment.slice(0, eqIndex).trim();
    const valuePartRaw = rawSegment.slice(eqIndex + 1);
    const valuePart = enumValue?.value ?? valuePartRaw.trim();

    if (/^\s*\\/.test(valuePartRaw)) {
      return createCommandItems(showAll ? '' : valuePartRaw.replace(/^\s*\\/, ''));
    }

    if (isReferenceDefinitionContext(context, argumentSpec)
      && [...getArgumentParameterTypes(argumentSpec, entry, key)].some(isReferenceType)) {
      return [];
    }

    const values = getArgumentParameterValues(argumentSpec, entry, key);
    if (!values || values.size === 0) {
      // In value position with free-form values, do not show key suggestions.
      return [];
    }

    return createValueItems(values, showAll ? '' : valuePart);
  }

  if (argumentSpec && argumentSpec.kind === 'assignments') {
    return createAssignmentItems(segment);
  }

  return [
    ...createValueItems(entry?.keywords || []),
    ...createAssignmentItems()
  ];
}

async function createArgumentCompletionItems(context, showAll = false) {
  if (!context) {
    return [];
  }

  const spec = getCommandArgumentSpec(context);
  const workspaceItems = await createWorkspaceArgumentItems(context, spec);
  if (!workspaceItems) {
    return createKeywordAndParameterItems(context, spec, showAll);
  }

  const rawSegment = getCurrentArgumentSegment(context).trim();
  const equalsIndex = rawSegment.indexOf('=');
  const filter = (equalsIndex >= 0 ? rawSegment.slice(equalsIndex + 1) : rawSegment)
    .trim()
    .toLowerCase();
  return workspaceItems.filter(item => showAll || !filter || item.label.toLowerCase().startsWith(filter));
}

  return {
    isEscaped,
    findMatchingDelimiter,
    mapCommandArguments,
    getBracketInvocationContext,
    getDocumentBracketContext,
    getTopLevelArgumentSegments,
    findAssignmentSeparator,
    getArgumentValueSpan,
    maskTeXComments,
    getCurrentAssignmentValue,
    setCompletionRange,
    createDynamicCompletionList,
    getCompletionRangeStart,
    getCompletionRangeEnd,
    triggerArgumentInformation,
    scheduleArgumentInformation,
    getCommandArgumentSpec,
    getCommandSignatureSpecs,
    getArgumentParameterNames,
    getArgumentParameterValues,
    getArgumentParameterTypes,
    initializeWorkspaceFileCache,
    isReferenceType,
    scanWorkspaceReferences,
    scanWorkspaceArgumentDiagnostics,
    createArgumentCompletionItems,
    getActiveAssignmentKey,
    createCommandItems,
    dispose: () => {
      clearTimeout(workspaceFileRefreshTimer);
      workspaceFileChangeListeners.forEach(listener => listener.dispose());
    }
  };
};
