/** Creates the VS Code activation entry point from explicit services. */
const createSyncTexActivationServices = require('./activation-synctex');
const registerSettingsTree = require('./settings-tree');
const registerMathEquationFeatures = require('./math-equations');
const { createWysiwymDecorations, extractSetting, inferCommandStyle, mergeStyles } = require('./wysiwym-decorations');

module.exports = function createActivator(deps) {
  const { isExistingFile, isConTeXtTexFilePath, toWorkspaceRelativePath, resolveConfiguredPath, normalizePathForComparison, inferredReferenceArguments, workspaceReferenceState } = deps;
  const { setDataWarningReporter } = deps;
  const { findMatchingDelimiter, maskTeXComments } = deps;
  const { getMacroParameterSemanticTokens, getStructureFoldingRanges, getCommandSemanticTokens, getArgumentSemanticTokens, getReferenceSemanticTokens, provideDocumentSymbols } = deps;
  const { vscode, fs, path, zlib, isContextTexDocument, environmentPairs, applyContextLanguageToOpenDocuments, findTexRootPathOnPath, collectWorkspaceTemporaryFiles, collectWorkspaceTemporaryFileState, temporaryFileExcludePatterns, temporaryFileWatcherPatterns, workspaceInterfaceWatcherPatterns, getAcademicPdfViewerExtension, formatSyncTexTracePath, formatSyncTexTraceUri, findOpenPdfGroup, findAnyPdfGroup, forgetClosedPdfTabs, runProcess, resolveSyncTexExecutable, resolveMtxRunExecutable, resolveXmlPathFromTexRoot, resolveContextExecutableFromTexRoot, resolveMainFilePath, openPdfFile, reloadActiveAcademicPdf, loadData, buildSignatureParts, getAcademicPdfViewerApi, parseSyncTexViewResult, parseSyncTexEditResult, parseMtxSyncTexFindResult, parseMtxSyncTexReportResult, triggerArgumentInformation, scheduleArgumentInformation, scanWorkspaceReferences, scanWorkspaceArgumentDiagnostics, initializeWorkspaceFileCache, createEmbeddedLuaBridge, getCommandSignatureSpecs, getActiveAssignmentKey, createCommandItems, createArgumentCompletionItems, getDocumentBracketContext, setCompletionRange, createDynamicCompletionList, getCompletionRangeStart, getCompletionRangeEnd, getCache } = deps;

  async function migrateLegacyConfiguration() {
    const mappings = [
      ['texRootPath', 'distribution.texRootPath'],
      ['mainFilePath', 'compilation.mainFilePath'],
      ['openPdfAfterCompile', 'compilation.openPdfAfterCompile'],
      ['hideTemporaryFiles', 'explorer.hideTemporaryFiles'],
      ['synctex', 'synctex.mode'],
      ['synctexmode', 'synctex.viewerMode'],
      ['formatting.color', 'fontSwitches.color']
    ];

    async function migrate(config, legacyKey, newKey, field, target) {
      const legacy = config.inspect(legacyKey)?.[field];
      if (legacy === undefined) return;
      try {
        if (config.inspect(newKey)?.[field] === undefined) {
          await config.update(newKey, legacy, target);
        }
      } catch (error) {
        console.warn(`Could not migrate ConTeXt IntelliSense setting ${legacyKey}:`, error);
      }
    }

    const config = vscode.workspace.getConfiguration('contextIntellisense');
    for (const [legacyKey, newKey] of mappings) {
      await migrate(config, legacyKey, newKey, 'globalValue', vscode.ConfigurationTarget.Global);
      await migrate(config, legacyKey, newKey, 'workspaceValue', vscode.ConfigurationTarget.Workspace);
      for (const folder of vscode.workspace.workspaceFolders || []) {
        const folderConfig = vscode.workspace.getConfiguration('contextIntellisense', folder.uri);
        await migrate(folderConfig, legacyKey, newKey, 'workspaceFolderValue', vscode.ConfigurationTarget.WorkspaceFolder);
      }
    }
  }

async function activate(context) {
  await migrateLegacyConfiguration();
  registerSettingsTree(vscode, context);
  registerMathEquationFeatures(vscode, context, getCache);

  const academicPdfViewerPromptKey = 'contextIntellisense.academicPdfViewerPromptShown';
  const outputChannel = vscode.window.createOutputChannel('ConTeXt');
  const syncTexOutputChannel = vscode.window.createOutputChannel('SyncTeX');
  const embeddedLua = createEmbeddedLuaBridge(vscode);
  const fileDecorationsEmitter = new vscode.EventEmitter();
  const codeLensesEmitter = new vscode.EventEmitter();
  const temporaryFilePaths = new Set();
  let temporaryFileRefreshTimer;
  let activeCompileCount = 0;
  let temporaryFileRefreshPending = false;
  let intellisenseDataLoad = Promise.resolve();
  let temporaryFileExclusionSync = Promise.resolve();
  const temporaryFileExcludesStateKey = 'contextIntellisense.temporaryFileExcludes';
  const compileDiagnostics = vscode.languages.createDiagnosticCollection('context-intellisense');
  const argumentDiagnostics = vscode.languages.createDiagnosticCollection('context-intellisense-arguments');
  const ignoredWarningsStateKey = 'contextIntellisense.ignoredWarnings';
  const ignoredWarnings = new Set(context.workspaceState.get(ignoredWarningsStateKey, []));
  const trackedArgumentDiagnostics = new Map();
  let argumentDiagnosticsRefreshTimer;
  let argumentDiagnosticsRefreshId = 0;
  const compileCurrentStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 103);
  compileCurrentStatus.name = 'Compile Current ConTeXt File';
  compileCurrentStatus.text = '$(play) Compile Current';
  compileCurrentStatus.tooltip = 'Compile the active ConTeXt file';
  compileCurrentStatus.command = 'contextIntellisense.compileCurrentFile';
  const compileMainStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 102);
  compileMainStatus.name = 'Compile Main ConTeXt File';
  compileMainStatus.text = '$(run-all) Compile Main';
  compileMainStatus.tooltip = 'Compile the configured main ConTeXt file';
  compileMainStatus.command = 'contextIntellisense.compileMainFile';
  const clearWorkspaceStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 101);
  clearWorkspaceStatus.name = 'Clear ConTeXt Workspace';
  clearWorkspaceStatus.text = '$(trash) Clear Workspace';
  clearWorkspaceStatus.tooltip = 'Delete temporary ConTeXt files from the workspace';
  clearWorkspaceStatus.command = 'contextIntellisense.clearWorkspace';
  context.subscriptions.push(outputChannel, syncTexOutputChannel, embeddedLua, fileDecorationsEmitter, codeLensesEmitter, compileDiagnostics, argumentDiagnostics, compileCurrentStatus, compileMainStatus, clearWorkspaceStatus);

  function warningKey(uri, diagnostic) {
    const { start, end } = diagnostic.range;
    return JSON.stringify([
      uri.toString(), diagnostic.source || '', diagnostic.code || '', diagnostic.message,
      start.line, start.character, end.line, end.character
    ]);
  }

  function visibleDiagnostics(uri, diagnostics) {
    return diagnostics.filter(diagnostic => diagnostic.severity !== vscode.DiagnosticSeverity.Warning
      || !ignoredWarnings.has(warningKey(uri, diagnostic)));
  }

  /** Starts one shared load for all IntelliSense providers. */
  function loadIntellisenseData(texRootPath = getConfiguredTexRootPath(), force = false) {
    intellisenseDataLoad = loadData(texRootPath, force);
    return intellisenseDataLoad;
  }

  /** Replaces diagnostics from the workspace argument validator. */
  function setArgumentDiagnostics(uri, diagnostics) {
    diagnostics = visibleDiagnostics(uri, diagnostics);
    if (diagnostics.length === 0) {
      argumentDiagnostics.delete(uri);
      trackedArgumentDiagnostics.delete(uri.toString());
      return;
    }
    argumentDiagnostics.set(uri, diagnostics);
    trackedArgumentDiagnostics.set(uri.toString(), uri);
  }

  /** Refreshes argument warnings and reloads interface data only when requested. */
  async function refreshWorkspaceArgumentDiagnostics(reloadIntellisenseData = false) {
    const refreshId = ++argumentDiagnosticsRefreshId;
    try {
      await (reloadIntellisenseData ? loadIntellisenseData(undefined, true) : intellisenseDataLoad);
      const results = await scanWorkspaceArgumentDiagnostics();
      if (refreshId !== argumentDiagnosticsRefreshId) {
        return;
      }
      const currentUris = new Set();
      for (const { uri, diagnostics } of results) {
        currentUris.add(uri.toString());
        setArgumentDiagnostics(uri, diagnostics);
      }
      for (const [key, uri] of trackedArgumentDiagnostics) {
        if (!currentUris.has(key)) {
          argumentDiagnostics.delete(uri);
          trackedArgumentDiagnostics.delete(key);
        }
      }
    } catch (error) {
      outputChannel.appendLine(`Could not validate ConTeXt arguments: ${error.message || error}`);
    }
  }

  /** Debounces a workspace-wide argument diagnostics refresh. */
  function scheduleWorkspaceArgumentDiagnostics(reloadIntellisenseData = false) {
    clearTimeout(argumentDiagnosticsRefreshTimer);
    argumentDiagnosticsRefreshTimer = setTimeout(() => {
      void refreshWorkspaceArgumentDiagnostics(reloadIntellisenseData);
    }, 300);
  }

  context.subscriptions.push({
    dispose: () => {
      clearTimeout(argumentDiagnosticsRefreshTimer);
      argumentDiagnosticsRefreshId += 1;
    }
  });

  /** Writes a complete JSON diagnostic record for one SyncTeX operation. */
  const syncTex = createSyncTexActivationServices({ context, outputChannel: syncTexOutputChannel, vscode, fs, path, zlib, isContextTexDocument, isExistingFile, normalizePathForComparison, toWorkspaceRelativePath, resolveConfiguredPath, resolveXmlPathFromTexRoot, resolveMainFilePath, resolveContextExecutableFromTexRoot, resolveSyncTexExecutable, resolveMtxRunExecutable, runProcess, parseSyncTexViewResult, parseSyncTexEditResult, parseMtxSyncTexFindResult, parseMtxSyncTexReportResult, getAcademicPdfViewerApi, openPdfFile, findOpenPdfGroup, formatSyncTexTracePath, formatSyncTexTraceUri });
const { writeSyncTexTrace, getWorkspaceRootPath, getSyncTexMode, getSyncTexIntegrationMode, getConfiguredTexRootPath, getConfiguredMainFilePath, resolveWorkspaceRelativePath, resolveViewerPdfPath, hasConfiguredTexRoot, getResolvedTexRootPath, getResolvedXmlPath, getResolvedContextExecutable, getResolvedMainFilePath, findSyncTexSidecar, ensureGzippedSyncTexSidecar, configureAcademicPdfViewerBridge, resolveSyncTexInputName, getSyncTexWorkingDirectory, toSyncTexRelativePath, resolveSyncTexSourcePath, resolveSyncTexPdfPath, buildSyncTexCommand, forwardSyncTex, findVisibleSourceEditor, findOpenSourceTab, findNormalEditorColumn, showSyncTexSource, handleInverseSyncTex, connectAcademicPdfViewerSyncTex } = syncTex;

  const doubleClickIntervalMs = 500;
  let pendingForwardClick;
  let pendingForwardClickTimer;

  /** Tests whether a word selection contains the preceding single-click position. */
  function isDoubleClickSelection(previous, current) {
    const selection = current.textEditor.selection;
    const position = previous.position;
    if (previous.documentUri !== current.textEditor.document.uri.toString() || selection.isEmpty) {
      return false;
    }

    return position.line === selection.start.line && position.line === selection.end.line
      && position.character >= selection.start.character
      && position.character <= selection.end.character;
  }

  /** Runs forward SyncTeX only after the second click of a double-click. */
  function handleDoubleClickSelection(event) {
    const now = Date.now();
    const selection = event.textEditor.selection;
    if (pendingForwardClick
      && now - pendingForwardClick.time <= doubleClickIntervalMs
      && isDoubleClickSelection(pendingForwardClick, event)) {
      clearTimeout(pendingForwardClickTimer);
      pendingForwardClick = undefined;
      pendingForwardClickTimer = undefined;
      void forwardSyncTex(event.textEditor, { trigger: 'doubleclick' }).catch((error) => {
        writeSyncTexTrace('forward', {
          status: 'exception',
          editorUri: event.textEditor.document.uri.toString(),
          error: String(error && error.stack ? error.stack : error)
        });
      });
      return;
    }

    clearTimeout(pendingForwardClickTimer);
    pendingForwardClick = selection.isEmpty
      ? {
        documentUri: event.textEditor.document.uri.toString(),
        position: { line: selection.active.line, character: selection.active.character },
        time: now
      }
      : undefined;
    pendingForwardClickTimer = pendingForwardClick
      ? setTimeout(() => {
        pendingForwardClick = undefined;
        pendingForwardClickTimer = undefined;
      }, doubleClickIntervalMs)
      : undefined;
  }

  function refreshDecorations() {
    fileDecorationsEmitter.fire();
    codeLensesEmitter.fire();
  }

  function setTemporaryFileDecorations(files) {
    const nextTemporaryFilePaths = new Set(
      files.map(normalizePathForComparison)
    );
    const changedPaths = [
      ...temporaryFilePaths,
      ...nextTemporaryFilePaths
    ].filter(filePath => temporaryFilePaths.has(filePath) !== nextTemporaryFilePaths.has(filePath));

    temporaryFilePaths.clear();
    for (const filePath of nextTemporaryFilePaths) {
      temporaryFilePaths.add(filePath);
    }
    if (changedPaths.length > 0) {
      fileDecorationsEmitter.fire(changedPaths.map(filePath => vscode.Uri.file(filePath)));
    }
  }

  function refreshTemporaryFileDecorations() {
    const { files, expectedPdfPaths } = collectWorkspaceTemporaryFileState(
      vscode.workspace.workspaceFolders || []
    );
    setTemporaryFileDecorations(files);
    temporaryFileExclusionSync = temporaryFileExclusionSync
      .then(() => synchronizeTemporaryFileExcludes(expectedPdfPaths))
      .then(() => true)
      .catch(error => {
        const message = `Could not update temporary-file Explorer exclusions: ${error.message || error}`;
        outputChannel.appendLine(message);
        vscode.window.setStatusBarMessage(`ConTeXt IntelliSense: ${message}`, 8000);
        return false;
      });
    return temporaryFileExclusionSync;
  }

  function escapeGlobSegment(segment) {
    const replacements = { '*': '[*]', '?': '[?]', '[': '[[]', ']': '[]]', '{': '[{]', '}': '[}]', '!': '[!]' };
    return segment.replace(/[*?\[\]{}!]/g, character => replacements[character]);
  }

  function sameSettingsRecord(left, right) {
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);
    return leftKeys.length === rightKeys.length
      && leftKeys.every(key => left[key] === right[key]);
  }

  async function synchronizeTemporaryFileExcludes(expectedPdfPaths) {
    const folders = (vscode.workspace.workspaceFolders || [])
      .filter(folder => folder.uri.scheme === 'file');
    const target = folders.length === 1
      ? vscode.ConfigurationTarget.Workspace
      : vscode.ConfigurationTarget.WorkspaceFolder;
    const state = { ...context.workspaceState.get(temporaryFileExcludesStateKey, {}) };

    for (const folder of folders) {
      const folderPath = path.resolve(folder.uri.fsPath);
      const folderKey = folder.uri.toString();
      const ownedPatterns = { ...(state[folderKey] || {}) };
      const desiredPatterns = new Set(temporaryFileExcludePatterns);
      if (vscode.workspace.getConfiguration('contextIntellisense').get('explorer.hideTemporaryFiles', false)) {
        for (const file of expectedPdfPaths) {
          const relativePath = path.relative(folderPath, path.resolve(file));
          if (relativePath === '..' || relativePath.startsWith(`..${path.sep}`) || path.isAbsolute(relativePath)) {
            continue;
          }
          desiredPatterns.add(
            relativePath.split(path.sep).map(escapeGlobSegment).join('/')
          );
        }
      } else {
        desiredPatterns.clear();
      }

      const configuration = vscode.workspace.getConfiguration('files', folder.uri);
      const inspected = configuration.inspect('exclude');
      const targetValue = target === vscode.ConfigurationTarget.WorkspaceFolder
        ? inspected?.workspaceFolderValue
        : inspected?.workspaceValue;
      const current = { ...(targetValue || {}) };
      const updated = { ...current };

      for (const [pattern, original] of Object.entries(ownedPatterns)) {
        if (desiredPatterns.has(pattern)) {
          continue;
        }
        if (updated[pattern] === true) {
          if (original.present) {
            updated[pattern] = original.value;
          } else {
            delete updated[pattern];
          }
        }
        delete ownedPatterns[pattern];
      }

      for (const pattern of desiredPatterns) {
        if (Object.prototype.hasOwnProperty.call(ownedPatterns, pattern)) {
          updated[pattern] = true;
        } else if (updated[pattern] !== true) {
          ownedPatterns[pattern] = {
            present: Object.prototype.hasOwnProperty.call(updated, pattern),
            value: updated[pattern]
          };
          updated[pattern] = true;
        }
      }

      if (!sameSettingsRecord(current, updated)) {
        await configuration.update(
          'exclude',
          Object.keys(updated).length > 0 ? updated : undefined,
          target
        );
      }
      if (Object.keys(ownedPatterns).length > 0) {
        state[folderKey] = ownedPatterns;
      } else {
        delete state[folderKey];
      }
      await context.workspaceState.update(temporaryFileExcludesStateKey, state);
    }
  }

  function scheduleTemporaryFileDecorationRefresh() {
    clearTimeout(temporaryFileRefreshTimer);
    if (activeCompileCount > 0) {
      temporaryFileRefreshPending = true;
      return;
    }
    temporaryFileRefreshTimer = setTimeout(() => {
      temporaryFileRefreshTimer = undefined;
      if (activeCompileCount > 0) {
        temporaryFileRefreshPending = true;
        return;
      }
      refreshTemporaryFileDecorations();
    }, 150);
  }

  function beginCompileFileDecorationBatch() {
    clearTimeout(temporaryFileRefreshTimer);
    temporaryFileRefreshTimer = undefined;
    activeCompileCount += 1;
    temporaryFileRefreshPending = true;
  }

  function endCompileFileDecorationBatch() {
    activeCompileCount = Math.max(0, activeCompileCount - 1);
    if (activeCompileCount === 0 && temporaryFileRefreshPending) {
      temporaryFileRefreshPending = false;
      refreshTemporaryFileDecorations();
    }
  }

  /** Returns the PDF path associated with a ConTeXt document. */
  function getPdfPathForDocument(document) {
    if (!document || !isContextTexDocument(document)) {
      return '';
    }
    return path.join(
      path.dirname(document.uri.fsPath),
      `${path.basename(document.uri.fsPath, path.extname(document.uri.fsPath))}.pdf`
    );
  }

  /** Updates dynamic visibility of the editor title-bar commands. */
  function updateCommandVisibility() {
    const editor = vscode.window.activeTextEditor;
    const pdfPath = getPdfPathForDocument(editor?.document);
    const pdfAvailable = !!pdfPath && isExistingFile(pdfPath);
    const mainFileAvailable = !!getResolvedMainFilePath();
    const isContextEditor = !!editor && isContextTexDocument(editor.document);
    const hasWorkspace = !!(vscode.workspace.workspaceFolders || []).length;

    isContextEditor ? compileCurrentStatus.show() : compileCurrentStatus.hide();
    mainFileAvailable ? compileMainStatus.show() : compileMainStatus.hide();
    hasWorkspace ? clearWorkspaceStatus.show() : clearWorkspaceStatus.hide();

    void vscode.commands.executeCommand(
      'setContext',
      'contextIntellisense.hasPdfForActiveFile',
      pdfAvailable
    );
    void vscode.commands.executeCommand(
      'setContext',
      'contextIntellisense.hasMainFile',
      mainFileAvailable
    );
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
    await config.update('distribution.texRootPath', texRootPath, vscode.ConfigurationTarget.Global);
    await loadIntellisenseData(texRootPath);
    refreshDecorations();

    const resolvedXmlPath = getResolvedXmlPath();
    if (!resolvedXmlPath) {
      publishCompileWarning(path.join(texRootPath, 'i-context.xml'), `i-context.xml was not found below ${texRootPath}.`);
    } else {
      vscode.window.setStatusBarMessage(`ConTeXt IntelliSense configured with TeX root: ${texRootPath}`, 4000);
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
      publishCompileProblem(selected[0].fsPath, 'The main file must be inside the current workspace.');
      return false;
    }
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    await config.update('compilation.mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
    refreshDecorations();
    vscode.window.setStatusBarMessage(`ConTeXt IntelliSense main file set to: ${mainFilePath}`, 4000);
    return true;
  }

  async function maybeRunFirstStartSetup(force = false) {
    const configuredTexRootPath = getConfiguredTexRootPath();
    if (hasConfiguredTexRoot() && !force) {
      return;
    }

    if (!force && !configuredTexRootPath) {
      const detectedTexRootPath = findTexRootPathOnPath();
      if (detectedTexRootPath) {
        const config = vscode.workspace.getConfiguration('contextIntellisense');
        await config.update('distribution.texRootPath', detectedTexRootPath, vscode.ConfigurationTarget.Global);
        await loadIntellisenseData(detectedTexRootPath);
        refreshDecorations();
        await maybeShowAcademicPdfViewerPrompt();
        return;
      }
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
      item.diagnostic.source = 'ConTeXt';
      item.diagnostic.code = 'compile-error';
      if (!grouped.has(item.uri.toString())) {
        grouped.set(item.uri.toString(), []);
      }
      grouped.get(item.uri.toString()).push(item.diagnostic);
    }
    for (const item of diagnostics) {
      compileDiagnostics.set(item.uri, grouped.get(item.uri.toString()));
    }
    if (diagnostics.length === 0) {
      const uri = vscode.Uri.file(targetFilePath);
      const diagnostic = new vscode.Diagnostic(
        new vscode.Range(0, 0, 0, 0),
        fallbackMessage,
        vscode.DiagnosticSeverity.Error
      );
      diagnostic.source = 'ConTeXt';
      diagnostic.code = 'compile-error';
      compileDiagnostics.set(uri, [diagnostic]);
      return { summary: fallbackMessage, diagnostics: [{ uri, diagnostic }] };
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

  function hasPdfLockMessage(output) {
    const text = String(output || '');
    return /sharing violation|being used by another process|\.pdf[^\r\n]*(?:locked|in use|access denied|permission denied)/i.test(text)
      || /(?:can't|cannot|could not|unable to|failed to)\s+(?:write(?:\s+on)?\s+file|open(?:ed)?|write|replace|rename)[^\r\n]*\.pdf/i.test(text)
      || /\.pdf[`'\"]?\s+(?:can't|cannot|could not|unable to|failed to)\s+(?:be\s+)?(?:open(?:ed)?|written|replaced|renamed)[^\r\n]*/i.test(text);
  }

  /** Publishes a compiler or extension problem with a stable Problems code. */
  function publishCompileProblem(filePath, message, severity = vscode.DiagnosticSeverity.Error, code = 'problem') {
    const uri = vscode.Uri.file(filePath);
    const diagnostic = new vscode.Diagnostic(
      new vscode.Range(0, 0, 0, 0),
      message,
      severity
    );
    diagnostic.source = 'ConTeXt';
    diagnostic.code = code;
    const visible = visibleDiagnostics(uri, [diagnostic]);
    if (visible.length > 0) compileDiagnostics.set(uri, visible);
    else compileDiagnostics.delete(uri);
  }

  async function ignoreWarning(uriText, key) {
    const uri = vscode.Uri.parse(uriText);
    ignoredWarnings.add(key);
    await context.workspaceState.update(ignoredWarningsStateKey, [...ignoredWarnings]);
    for (const collection of [compileDiagnostics, argumentDiagnostics]) {
      const remaining = (collection.get(uri) || []).filter(diagnostic => warningKey(uri, diagnostic) !== key);
      if (remaining.length > 0) collection.set(uri, remaining);
      else collection.delete(uri);
    }
  }

  context.subscriptions.push(vscode.commands.registerCommand(
    'contextIntellisense.ignoreWarning', ignoreWarning
  ));
  context.subscriptions.push(vscode.commands.registerCommand(
    'contextIntellisense.clearIgnoredWarnings', async () => {
      ignoredWarnings.clear();
      await context.workspaceState.update(ignoredWarningsStateKey, []);
      scheduleWorkspaceArgumentDiagnostics(true);
    }
  ));

  function publishCompileWarning(filePath, message) {
    publishCompileProblem(filePath, message, vscode.DiagnosticSeverity.Warning, 'warning');
  }

  setDataWarningReporter?.((filePath, message) => {
    publishCompileWarning(filePath, message);
  });

  function readTextIfExists(filePath) {
    try {
      return isExistingFile(filePath) ? fs.readFileSync(filePath, 'utf8') : '';
    } catch (error) {
      return '';
    }
  }

  async function compileTargetFile(targetFilePath, title) {
    const resolvedTargetPath = resolveConfiguredPath(targetFilePath, getWorkspaceRootPath());
    if (!isExistingFile(resolvedTargetPath)) {
      publishCompileProblem(resolvedTargetPath, `Target file not found: ${resolvedTargetPath}`);
      return false;
    }

    const contextExecutable = getResolvedContextExecutable();
    if (!contextExecutable) {
      publishCompileProblem(resolvedTargetPath, 'Could not locate the ConTeXt executable. Set the TeX root folder first.');
      return false;
    }

    const cwd = path.dirname(resolvedTargetPath);
    const pdfPath = path.join(cwd, `${path.basename(resolvedTargetPath, path.extname(resolvedTargetPath))}.pdf`);
    const contextLogPath = path.join(cwd, `${path.basename(resolvedTargetPath, path.extname(resolvedTargetPath))}.log`);
    const errorLogPath = path.join(cwd, `${path.basename(resolvedTargetPath, path.extname(resolvedTargetPath))}-error.log`);

    compileDiagnostics.delete(vscode.Uri.file(resolvedTargetPath));
    outputChannel.clear();
    const compileArgs = ['--synctex', resolvedTargetPath];
    let compilationFailure = null;
    let compilationWarning = false;
    beginCompileFileDecorationBatch();
    try {
      await vscode.window.withProgress({
        location: vscode.ProgressLocation.Window,
        title,
        cancellable: false
      }, async () => {
        const result = await runProcess(contextExecutable, compileArgs, cwd, outputChannel);
        const processOutput = [
          result.stdout || '',
          result.stderr || '',
          readTextIfExists(contextLogPath),
          readTextIfExists(errorLogPath)
        ].join('\n');
        if (hasPdfLockMessage(processOutput)) {
          compilationWarning = true;
          publishCompileWarning(resolvedTargetPath, `ConTeXt could not write the output PDF. It may be locked by an external viewer. Close it in the viewer and compile again: ${pdfPath}`);
          return;
        }

        const logHasError = hasCompileErrorLog(errorLogPath);
        if (result.error || result.code !== 0 || logHasError) {
          const reason = result.error
            ? result.error.message || String(result.error)
            : result.code !== 0
              ? `exit code ${result.code}`
              : 'the ConTeXt error log reports an error';
          compilationFailure = publishCompileDiagnostics(errorLogPath, resolvedTargetPath, reason);
          return;
        }

        if (!isExistingFile(pdfPath)) {
          compilationWarning = true;
          publishCompileWarning(resolvedTargetPath, `ConTeXt did not produce an output PDF: ${pdfPath}`);
          return;
        }

        compileDiagnostics.clear();
        vscode.window.setStatusBarMessage(
          `$(check) ConTeXt compilation succeeded: ${path.basename(resolvedTargetPath)}`,
          4000
        );
        if (getSyncTexIntegrationMode() === 'bridge') {
          try {
            const compressed = await ensureGzippedSyncTexSidecar(pdfPath);
            if (compressed) {
              await configureAcademicPdfViewerBridge(pdfPath);
            }
          } catch (error) {
            syncTexOutputChannel.appendLine(`SyncTeX bridge setup failed: ${error.message || error}`);
          }
        }
        await reloadActiveAcademicPdf();
        const openPdfAfterCompile = vscode.workspace
          .getConfiguration('contextIntellisense')
          .get('compilation.openPdfAfterCompile', true);
        if (openPdfAfterCompile) {
          try {
            await openPdfFile(pdfPath);
          } catch (error) {
            publishCompileWarning(resolvedTargetPath, `Compilation succeeded, but the PDF could not be opened: ${error.message || error}`);
          }
        }
      });
    } finally {
      endCompileFileDecorationBatch();
    }

    if (compilationFailure) {
      await vscode.commands.executeCommand('workbench.actions.view.problems');
    }

    if (compilationWarning) {
      return false;
    }

    return true;
  }

  async function compileActiveOrProvidedDocument(targetUri) {
    const uri = targetUri || (vscode.window.activeTextEditor && vscode.window.activeTextEditor.document.uri);
    if (!uri) {
      vscode.window.setStatusBarMessage('ConTeXt IntelliSense: no active document to compile.', 4000);
      return false;
    }

    const document = await vscode.workspace.openTextDocument(uri);
    if (!isContextTexDocument(document)) {
      publishCompileProblem(document.uri.fsPath, 'Compile is only available for ConTeXt TEX files.');
      return false;
    }

    if (document.isDirty) {
      const saved = await document.save();
      if (!saved) {
        publishCompileProblem(document.uri.fsPath, `Could not save ${document.fileName}.`);
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
        publishCompileProblem(attemptedPath, `Configured main file not found: ${attemptedPath}`);
        return false;
      }

      vscode.window.setStatusBarMessage('ConTeXt IntelliSense: no main file configured. Set one from Explorer or the command palette.', 5000);
      return false;
    }

    for (const document of vscode.workspace.textDocuments) {
      if (isContextTexDocument(document) && document.isDirty && !await document.save()) {
        publishCompileProblem(document.uri.fsPath, `Could not save ${document.fileName}.`);
        return false;
      }
    }

    return compileTargetFile(mainFilePath, 'Compile main ConTeXt file');
  }

  async function showPdfForActiveDocument() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isContextTexDocument(editor.document)) {
      if (editor) {
        publishCompileProblem(editor.document.uri.fsPath, 'Open PDF is only available for ConTeXt TEX documents.');
      } else {
        vscode.window.setStatusBarMessage('ConTeXt IntelliSense: no active ConTeXt TEX document.', 4000);
      }
      return false;
    }

    const pdfPath = getPdfPathForDocument(editor.document);

    if (!isExistingFile(pdfPath)) {
      publishCompileWarning(editor.document.uri.fsPath, `PDF not found: ${pdfPath}`);
      return false;
    }

    try {
      await openPdfFile(pdfPath, { forceFocus: true });
      return true;
    } catch (error) {
      publishCompileWarning(editor.document.uri.fsPath, `Could not open PDF: ${error.message || error}`);
      return false;
    }
  }

  async function clearWorkspaceTemporaryFiles() {
    const workspaceFolders = vscode.workspace.workspaceFolders || [];
    if (workspaceFolders.length === 0) {
      vscode.window.setStatusBarMessage('ConTeXt IntelliSense: no workspace folder is open.', 4000);
      return false;
    }

    const filesToDelete = collectWorkspaceTemporaryFiles(workspaceFolders);
    setTemporaryFileDecorations(filesToDelete);
    if (filesToDelete.length === 0) {
      vscode.window.setStatusBarMessage('ConTeXt IntelliSense: no temporary files found.', 4000);
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
    refreshTemporaryFileDecorations();

    const deletedCount = filesToDelete.length - failures.length;
    if (failures.length > 0) {
      for (const failure of failures) {
        publishCompileWarning(failure.filePath, `Could not delete temporary file: ${failure.error.message || failure.error}`);
      }
      return false;
    }

    vscode.window.setStatusBarMessage(`ConTeXt IntelliSense: deleted ${deletedCount} temporary files.`, 4000);
    return true;
  }

  async function setTemporaryFilesHidden(hidden) {
    try {
      await vscode.workspace.getConfiguration('contextIntellisense').update(
        'explorer.hideTemporaryFiles',
        hidden,
        vscode.ConfigurationTarget.Workspace
      );
      clearTimeout(temporaryFileRefreshTimer);
      temporaryFileRefreshTimer = undefined;
      if (!await refreshTemporaryFileDecorations()) {
        return;
      }
      vscode.window.setStatusBarMessage(
        `ConTeXt IntelliSense: temporary files ${hidden ? 'hidden' : 'shown'} in Explorer.`,
        4000
      );
    } catch (error) {
      vscode.window.setStatusBarMessage(
        `ConTeXt IntelliSense: could not change temporary-file visibility: ${error.message || error}`,
        8000
      );
    }
  }

  void refreshWorkspaceArgumentDiagnostics(true);
  void initializeWorkspaceFileCache().catch(error => {
    outputChannel.appendLine(`Could not initialize workspace file completions: ${error.message || error}`);
  });
  void applyContextLanguageToOpenDocuments().then(() => updateEnvironmentPairHighlights());
  const configureTexRootCommand = vscode.commands.registerCommand('contextIntellisense.configureTexRootPath', async () => {
    await maybeRunFirstStartSetup(true);
  });
  context.subscriptions.push(configureTexRootCommand);

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
      vscode.window.setStatusBarMessage('ConTeXt IntelliSense: select a ConTeXt TEX file in Explorer.', 4000);
      return;
    }

    if (!isConTeXtTexFilePath(candidateUri.fsPath)) {
      publishCompileProblem(candidateUri.fsPath, 'Selected file is not a ConTeXt TEX file.');
      return;
    }

    const mainFilePath = toWorkspaceRelativePath(candidateUri.fsPath, getWorkspaceRootPath());
    if (!mainFilePath) {
      publishCompileProblem(candidateUri.fsPath, 'The main file must be inside the current workspace.');
      return;
    }
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    await config.update('compilation.mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
    refreshDecorations();
    vscode.window.setStatusBarMessage(`ConTeXt IntelliSense main file set to: ${mainFilePath}`, 4000);
  });
  context.subscriptions.push(setMainFromExplorerCommand);

  const compileCurrentCommand = vscode.commands.registerCommand('contextIntellisense.compileCurrentFile', async (targetUri) => {
    await compileActiveOrProvidedDocument(targetUri);
  });
  context.subscriptions.push(compileCurrentCommand);

  function getSelectedTextTarget() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isContextTexDocument(editor.document)) return undefined;
    const selections = editor.selections.filter(selection => !selection.isEmpty);
    return selections.length > 0
      ? { editor, document: editor.document, version: editor.document.version, selections }
      : undefined;
  }

  async function applySelectedEdits(target, changes) {
    if (!target || target.editor.document !== target.document
      || target.document.version !== target.version) return;
    const document = target.document;
    const edits = changes.map(change => ({
      ...change,
      start: document.offsetAt(change.range.start),
      end: document.offsetAt(change.range.end)
    })).sort((a, b) => a.start - b.start || b.end - a.end);
    const nonOverlapping = [];
    for (const change of edits) {
      if (!nonOverlapping.some(previous => change.start < previous.end && previous.start < change.end)) {
        nonOverlapping.push(change);
      }
    }
    const applied = await target.editor.edit(edit => {
      for (const change of nonOverlapping) edit.replace(change.range, change.text);
    });
    if (!applied || !nonOverlapping.length
      || !nonOverlapping.every(change => Number.isInteger(change.selectionStart) && Number.isInteger(change.selectionEnd))) return;
    let offsetDelta = 0;
    target.editor.selections = nonOverlapping.map(change => {
      const start = change.start + offsetDelta;
      offsetDelta += change.text.length - (change.end - change.start);
      return new vscode.Selection(
        document.positionAt(start + change.selectionStart),
        document.positionAt(start + change.selectionEnd)
      );
    });
  }

  function getTemplateTextRange(template, values, textLength) {
    const markerIndex = normalizeTemplate(template).indexOf('${text}');
    const start = markerIndex < 0 ? 0 : expandTemplate(normalizeTemplate(template).slice(0, markerIndex), values).length;
    return { selectionStart: start, selectionEnd: start + textLength };
  }

  async function wrapSelectedText(target, template, values = {}) {
    if (!target) return;
    return applySelectedEdits(target, target.selections.map(selection => {
      const text = target.document.getText(selection);
      return {
        range: selection,
        text: expandTemplate(template, { ...values, text }),
        ...getTemplateTextRange(template, values, text.length)
      };
    }));
  }

  function normalizeTemplate(template) {
    return typeof template === 'string' ? template.replaceAll('\\*', '${text}') : '';
  }

  function expandTemplate(template, values) {
    return normalizeTemplate(template).replace(/\$\{([^}]+)\}/g,
      (placeholder, name) => Object.prototype.hasOwnProperty.call(values, name) ? values[name] : placeholder);
  }

  function hasOneTextPlaceholder(template) {
    return normalizeTemplate(template).split('${text}').length === 2;
  }

  function hasBalancedBraces(value) {
    let depth = 0;
    for (let index = 0; index < value.length; index += 1) {
      const character = value[index];
      if (character === '\\') {
        index += 1;
      } else if (character === '%') {
        while (index < value.length && value[index] !== '\n') index += 1;
      } else if (character === '{') {
        depth += 1;
      } else if (character === '}' && --depth < 0) {
        return false;
      }
    }
    return depth === 0;
  }

  function findTemplateWrapper(document, selection, template) {
    const normalized = normalizeTemplate(template);
    if (!hasOneTextPlaceholder(normalized) || selection.start.line !== selection.end.line) return undefined;
    const markerIndex = normalized.indexOf('${text}');
    const prefix = normalized.slice(0, markerIndex);
    const suffix = normalized.slice(markerIndex + '${text}'.length);
    if (!prefix || !suffix) return undefined;

    const line = document.lineAt(selection.start.line);
    const source = line.text;
    const selectionStart = selection.start.character;
    const selectionEnd = selection.end.character;
    let wrapperStart = source.lastIndexOf(prefix, Math.min(selectionEnd - 1, selectionStart + prefix.length - 1));
    while (wrapperStart >= 0) {
      if (wrapperStart > selectionStart) {
        wrapperStart = wrapperStart === 0 ? -1 : source.lastIndexOf(prefix, wrapperStart - 1);
        continue;
      }
      let suffixStart = source.indexOf(suffix, wrapperStart + prefix.length);
      while (suffixStart >= 0 && !hasBalancedBraces(source.slice(wrapperStart, suffixStart + suffix.length))) {
        suffixStart = source.indexOf(suffix, suffixStart + 1);
      }
      const wrapperEnd = suffixStart < 0 ? -1 : suffixStart + suffix.length;
      if (wrapperEnd >= selectionEnd) {
        const lineStart = document.offsetAt(new vscode.Position(selection.start.line, 0));
        const start = lineStart + wrapperStart;
        const end = lineStart + wrapperEnd;
        return {
          range: new vscode.Range(document.positionAt(start), document.positionAt(end)),
          start,
          end,
          text: source.slice(wrapperStart + prefix.length, suffixStart)
        };
      }
      wrapperStart = wrapperStart === 0 ? -1 : source.lastIndexOf(prefix, wrapperStart - 1);
    }
    return undefined;
  }

  function toggleSelectionTemplate(target, template) {
    if (!target || !hasOneTextPlaceholder(template)) return;
    return applySelectedEdits(target, target.selections.map(selection => {
      const wrapper = findTemplateWrapper(target.document, selection, template);
      if (wrapper) return {
        range: wrapper.range,
        text: wrapper.text,
        selectionStart: 0,
        selectionEnd: wrapper.text.length
      };
      const text = target.document.getText(selection);
      return {
        range: selection,
        text: expandTemplate(template, { text }),
        ...getTemplateTextRange(template, {}, text.length)
      };
    }));
  }

  function applyTemplateToSelection(target, template, values = {}) {
    const normalizedTemplate = normalizeTemplate(template);
    if (!normalizedTemplate.includes('${text}')) return;
    return wrapSelectedText(target, template, values);
  }

  function getConfiguredTemplate(setting, fallback) {
    return vscode.workspace.getConfiguration('contextIntellisense').get(setting, fallback);
  }

  function applySelectionTemplate(target, setting, fallback, values = {}) {
    return applyTemplateToSelection(target, getConfiguredTemplate(setting, fallback), values);
  }

  const boldSelectionCommand = vscode.commands.registerCommand(
    'contextIntellisense.formatSelectionBold',
    () => toggleSelectionTemplate(getSelectedTextTarget(), getConfiguredTemplate('fontSwitches.bold', '\\bold{${text}}'))
  );
  const emphasizeSelectionCommand = vscode.commands.registerCommand(
    'contextIntellisense.formatSelectionEmphasize',
    () => toggleSelectionTemplate(getSelectedTextTarget(), getConfiguredTemplate('fontSwitches.emphasize', '\\emph{${text}}'))
  );
  const legacyFontSizeSettings = {
    xx: 'fontSizeXx', x: 'fontSizeX', normal: 'fontSizeNormal',
    a: 'fontSizeA', b: 'fontSizeB', c: 'fontSizeC', d: 'fontSizeD'
  };

  function getFontSizeTemplate(name, legacySetting) {
    const config = vscode.workspace.getConfiguration('contextIntellisense');
    const configuredSizes = config.get('fontSwitches.fontSizes', {});
    const sizeOverrides = config.inspect('fontSwitches.fontSizes');
    const hasCurrentValue = ['workspaceFolderValue', 'workspaceValue', 'globalValue']
      .some(target => Object.prototype.hasOwnProperty.call(sizeOverrides?.[target] || {}, name));
    if (!hasCurrentValue && legacySetting) {
      const legacyValues = config.inspect(`fontSwitches.${legacySetting}`);
      const legacyTemplate = ['workspaceFolderValue', 'workspaceValue', 'globalValue']
        .map(target => legacyValues?.[target])
        .find(value => typeof value === 'string');
      if (legacyTemplate !== undefined) return legacyTemplate;
    }
    return configuredSizes[name];
  }

  function getFontSizeEntries() {
    const sizes = vscode.workspace.getConfiguration('contextIntellisense').get('fontSwitches.fontSizes', {});
    return Object.keys(sizes).map(name => ({
      label: name,
      template: getFontSizeTemplate(name, legacyFontSizeSettings[name])
    })).filter(entry => hasOneTextPlaceholder(entry.template));
  }

  function findFontSizeWrapper(document, selection, entries) {
    for (const entry of entries) {
      const wrapper = findTemplateWrapper(document, selection, entry.template);
      if (wrapper) return { ...wrapper, name: entry.label };
    }
    return undefined;
  }

  async function toggleFontSizeSelection() {
    const target = getSelectedTextTarget();
    if (!target) return;
    const entries = getFontSizeEntries();
    const current = target.selections.map(selection => findFontSizeWrapper(target.document, selection, entries));
    const currentName = current.length && current.every(wrapper => wrapper?.name === current[0]?.name)
      ? current[0]?.name
      : undefined;
    const options = entries.map(entry => ({
      label: entry.label,
      description: entry.label === currentName ? 'Current' : undefined
    }));
    const selected = await vscode.window.showQuickPick(options, { placeHolder: 'Select configured font size' });
    if (!selected || target.editor.document !== target.document || target.document.version !== target.version) return;
    const entry = entries.find(candidate => candidate.label === selected.label);
    if (!entry) return;
    await applySelectedEdits(target, target.selections.map((selection, index) => {
      const wrapper = current[index];
      if (wrapper?.name === entry.label) return {
        range: wrapper.range,
        text: wrapper.text,
        selectionStart: 0,
        selectionEnd: wrapper.text.length
      };
      const text = wrapper?.text ?? target.document.getText(selection);
      return {
        range: wrapper?.range ?? selection,
        text: expandTemplate(entry.template, { text }),
        ...getTemplateTextRange(entry.template, {}, text.length)
      };
    }));
  }

  const fontSizeCommand = vscode.commands.registerCommand('contextIntellisense.formatSelectionFontSize', toggleFontSizeSelection);
  function collectColorDefinitions(source, colors) {
    const uncommented = stripTeXComments(source);
    for (const [, name, value] of uncommented.matchAll(/\\define(?:global)?color\s*\[([^\]]+)\]\s*\[([^\]]+)\]/gi)) {
      colors.set(name.trim(), value.trim());
    }
  }

  function stripTeXComments(source) {
    return source.replace(/(^|[^\\])%[^\r\n]*/gm, '$1');
  }

  async function getWorkspaceEnvironmentSources() {
    const files = await vscode.workspace.findFiles(
      '**/*.{tex,mkiv,mkvi,mkxl,mklx,mkxi,mkil,mkix}',
      '**/{.git,node_modules,build,out}/**'
    );
    const sources = [];
    for (const file of files) {
      try {
        const source = await fs.promises.readFile(file.fsPath, 'utf8');
        const uncommented = stripTeXComments(source);
        if (/^env(?:ironment)?(?:[-_.]|$)/i.test(path.basename(file.fsPath))
          || /\\startenvironment\b/i.test(uncommented)) {
          sources.push({ source: uncommented, path: vscode.workspace.asRelativePath(file) });
        }
      } catch {
        continue;
      }
    }
    return sources;
  }

  async function getWorkspaceHighlightEntries() {
    const definitions = new Map();
    for (const { source } of await getWorkspaceEnvironmentSources()) {
      for (const [, names, settings = ''] of source.matchAll(/\\definehighlight\s*\[([^\]]+)\](?:\s*\[([^\]]*)\])?/gi)) {
        for (const rawName of names.split(',')) {
          const name = rawName.trim().replace(/^\\/, '');
          if (!/^[A-Za-z@:_][A-Za-z\d@:_-]*$/.test(name)) continue;
          const definition = definitions.get(name) || new Set();
          if (settings.trim()) definition.add(settings.trim());
          definitions.set(name, definition);
        }
      }
    }
    return [...definitions].sort(([left], [right]) => left.localeCompare(right)).map(([name, settings]) => ({
      name,
      template: `\\${name}{\${text}}`,
      settings: [...settings]
    }));
  }

  function resolveColorHex(name, definitions, seen = new Set()) {
    if (seen.has(name)) return undefined;
    const value = definitions.get(name);
    if (!value) return undefined;
    seen.add(name);
    const hex = value.match(/^#?([\da-f]{3}|[\da-f]{6})$/i)?.[1];
    if (hex) return hex.length === 3 ? [...hex].map(char => char + char).join('').toLowerCase() : hex.toLowerCase();
    const components = new Map([...value.matchAll(/(?:^|,)\s*([rgbhxskcmy])\s*=\s*([^,]+)/gi)]
      .map(([, key, component]) => [key.toLowerCase(), component.trim()]));
    const encodedHex = (components.get('x') || components.get('h'))?.match(/^#?([\da-f]{3}|[\da-f]{6})$/i)?.[1];
    if (encodedHex) return encodedHex.length === 3
      ? [...encodedHex].map(char => char + char).join('').toLowerCase()
      : encodedHex.toLowerCase();
    const unit = raw => {
      const number = Number.parseFloat(raw);
      if (!Number.isFinite(number)) return undefined;
      return Math.max(0, Math.min(1, raw.endsWith('%') ? number / 100 : number > 1 ? number / 255 : number));
    };
    const toHex = values => values.map(component => Math.round(component * 255).toString(16).padStart(2, '0')).join('');
    const gray = components.has('s') ? unit(components.get('s')) : undefined;
    if (gray !== undefined) return toHex([gray, gray, gray]);
    if (['r', 'g', 'b'].every(key => components.has(key))) {
      const rgb = ['r', 'g', 'b'].map(key => unit(components.get(key)));
      if (rgb.every(component => component !== undefined)) return toHex(rgb);
    }
    if (['c', 'm', 'y', 'k'].every(key => components.has(key))) {
      const cmyk = ['c', 'm', 'y', 'k'].map(key => unit(components.get(key)));
      if (cmyk.every(component => component !== undefined)) {
        const [cyan, magenta, yellow, black] = cmyk;
        return toHex([1 - Math.min(1, cyan + black), 1 - Math.min(1, magenta + black), 1 - Math.min(1, yellow + black)]);
      }
    }
    const alias = value.match(/^\s*([\w:-]+)\s*$/)?.[1];
    return alias ? resolveColorHex(alias, definitions, seen) : undefined;
  }

  async function getFormattingColorData() {
    await intellisenseDataLoad;
    const xmlPath = getCache().xmlPath;
    const contextRoot = xmlPath ? path.dirname(path.dirname(path.dirname(xmlPath))) : '';
    const standardFiles = contextRoot ? [
      path.join(contextRoot, 'base', 'mkiv', 'colo-imp-rgb.mkiv'),
      path.join(contextRoot, 'base', 'mkxl', 'colo-ini.mkxl'),
      path.join(contextRoot, 'base', 'mkiv', 'colo-ini.mkiv')
    ] : [];
    const standardColors = new Map();
    for (const file of standardFiles) {
      if (fs.existsSync(file)) collectColorDefinitions(fs.readFileSync(file, 'utf8'), standardColors);
    }
    const workspaceColors = new Map();
    for (const { source } of await getWorkspaceEnvironmentSources()) collectColorDefinitions(source, workspaceColors);
    const definitions = new Map([...standardColors, ...workspaceColors]);
    const names = workspaceColors.size ? [...workspaceColors.keys()] : [...standardColors.keys()];
    const colors = names.length ? names : ['black', 'white', 'red', 'green', 'blue', 'cyan', 'magenta', 'yellow'];
    return {
      definitions,
      colors: colors.sort((a, b) => a.localeCompare(b)).map(name => ({
        label: name,
        description: `#${resolveColorHex(name, definitions) || '808080'}`,
        hex: resolveColorHex(name, definitions) || '808080'
      }))
    };
  }

  async function getFormattingColors() {
    return (await getFormattingColorData()).colors;
  }

  async function createColorIcon(hex) {
    const storage = context.globalStorageUri.fsPath;
    await fs.promises.mkdir(storage, { recursive: true });
    const iconPath = path.join(storage, `color-${hex}.svg`);
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="1" y="1" width="14" height="14" rx="3" fill="#${hex}" stroke="#888"/></svg>`;
    await fs.promises.writeFile(iconPath, svg, 'utf8');
    return vscode.Uri.file(iconPath);
  }

  async function createColorPickerItems(colors) {
    return Promise.all(colors.map(async ({ label, hex }) => ({
      label,
      description: `#${hex}`,
      iconPath: await createColorIcon(hex)
    })));
  }

  function getHighlightColorHex(settings, definitions) {
    for (const setting of settings) {
      const value = setting.match(/(?:^|,)\s*color\s*=\s*(?:\{([^{}]*)\}|([^,\]]+))/i);
      const name = (value?.[1] || value?.[2])?.trim();
      if (!name) continue;
      const directHex = name.match(/^#?([\da-f]{3}|[\da-f]{6})$/i)?.[1];
      if (directHex) return directHex.length === 3
        ? [...directHex].map(character => character + character).join('').toLowerCase()
        : directHex.toLowerCase();
      return resolveColorHex(name, definitions)
        || resolveColorHex('__highlight_color__', new Map([...definitions, ['__highlight_color__', name]]));
    }
    return undefined;
  }

  function getHighlightStyleIcon(settings) {
    const styles = settings.flatMap(setting => [...setting.matchAll(/(?:^|,)\s*style\s*=\s*(?:\{([^{}]*)\}|([^,\]]+))/gi)]
      .map(([, braced, plain]) => braced || plain || ''));
    const commands = styles.flatMap(style => [...style.matchAll(/\\([A-Za-z]+)/g)].map(([, name]) => name.toLowerCase()));
    if (commands.some(name => /^(?:em(?:ph)?|(?:it|sl|bi|bs)(?:xx|x|[abcd])?|slanted|bolditalic|italicbold|s(?:lanted|tanted)bold|boldslanted)$/.test(name))) {
      return 'italic';
    }
    if (commands.some(name => /^(?:bold|bf(?:xx|x|[abcd])?)$/.test(name))) return 'bold';
    if (commands.some(name => /^(?:rm|ss|tt|mr|hw|cg)$/.test(name))) return 'symbol-text';
    return undefined;
  }

  const wysiwymDecorations = createWysiwymDecorations({
    vscode,
    isContextTexDocument,
    getCache,
    findMatchingDelimiter,
    maskTeXComments,
    async loadRules() {
      const [entries, colorData] = await Promise.all([getWorkspaceHighlightEntries(), getFormattingColorData()]);
      const commandStyles = new Map();
      for (const [setting, fallback, style] of [
        ['fontSwitches.bold', '\\bold{${text}}', { bold: true }],
        ['fontSwitches.emphasize', '\\emph{${text}}', { italic: true }]
      ]) {
        const template = getConfiguredTemplate(setting, fallback);
        for (const [, name] of template.matchAll(/\\([A-Za-z@]+)/g)) {
          commandStyles.set(name, mergeStyles(commandStyles.get(name), style));
        }
      }
      const colorTemplate = getConfiguredTemplate('fontSwitches.color', '\\color[${color}]{${text}}');
      const colorCommands = new Set([...colorTemplate.matchAll(/\\([A-Za-z@]+)/g)].map(([, name]) => name));
      const highlights = new Map(entries.map(entry => {
        const styleCommands = extractSetting(entry.settings, 'style').matchAll(/\\([A-Za-z@]+)/g);
        let style = {};
        for (const [, name] of styleCommands) {
          style = mergeStyles(style, commandStyles.get(name) || inferCommandStyle(name));
        }
        const color = getHighlightColorHex(entry.settings, colorData.definitions);
        if (color) style = mergeStyles(style, { color: `#${color}` });
        return [entry.name, style];
      }).filter(([, style]) => Object.keys(style).length));
      return {
        commandStyles,
        highlights,
        colorCommands,
        resolveColor(name) {
          const hex = String(name || '').match(/^#?([\da-f]{3}|[\da-f]{6})$/i)?.[1];
          if (hex) return hex.length === 3 ? [...hex].map(char => char + char).join('').toLowerCase() : hex.toLowerCase();
          return resolveColorHex(name, colorData.definitions);
        }
      };
    }
  });
  context.subscriptions.push({ dispose: () => wysiwymDecorations.dispose() });
  void applyContextLanguageToOpenDocuments().then(() => wysiwymDecorations.refresh());

  const colorSelectionCommand = vscode.commands.registerCommand(
    'contextIntellisense.formatSelectionColor',
    async () => {
      const target = getSelectedTextTarget();
      if (!target) return;
      const colors = await getFormattingColors();
      const colorItems = await createColorPickerItems(colors);
      const color = await vscode.window.showQuickPick(colorItems, { placeHolder: 'ConTeXt text color' });
      if (color) await applySelectionTemplate(target, 'fontSwitches.color', '\\color[${color}]{${text}}', { color: color.label });
    }
  );
  const highlightSelectionCommand = vscode.commands.registerCommand(
    'contextIntellisense.formatSelectionHighlight',
    async () => {
      const target = getSelectedTextTarget();
      if (!target) return;
      const [entries, colorData] = await Promise.all([getWorkspaceHighlightEntries(), getFormattingColorData()]);
      if (!entries.length || target.editor.document !== target.document || target.document.version !== target.version) return;
      const current = target.selections.map(selection => entries
        .map(entry => ({ entry, wrapper: findTemplateWrapper(target.document, selection, entry.template) }))
        .find(candidate => candidate.wrapper));
      const currentName = current.length && current.every(item => item?.entry.name === current[0]?.entry.name)
        ? current[0]?.entry.name
        : undefined;
      const options = await Promise.all(entries.map(async entry => {
        const hex = getHighlightColorHex(entry.settings, colorData.definitions);
        const styleIcon = hex ? undefined : getHighlightStyleIcon(entry.settings);
        return {
          label: `${styleIcon ? `$(${styleIcon}) ` : ''}\\${entry.name}`,
          description: entry.name === currentName ? 'Current' : undefined,
          detail: entry.settings.join(' · ') || undefined,
          iconPath: hex ? await createColorIcon(hex) : undefined,
          entry
        };
      }));
      const selected = await vscode.window.showQuickPick(options, { placeHolder: 'Select a defined ConTeXt highlight' });
      if (!selected || target.editor.document !== target.document || target.document.version !== target.version) return;
      await applySelectedEdits(target, target.selections.map((selection, index) => {
        const wrapper = current[index]?.wrapper;
        if (current[index]?.entry.name === selected.entry.name) return {
          range: wrapper.range,
          text: wrapper.text,
          selectionStart: 0,
          selectionEnd: wrapper.text.length
        };
        const text = wrapper?.text ?? target.document.getText(selection);
        return {
          range: wrapper?.range ?? selection,
          text: expandTemplate(selected.entry.template, { text }),
          ...getTemplateTextRange(selected.entry.template, {}, text.length)
        };
      }));
    }
  );
  context.subscriptions.push(
    boldSelectionCommand,
    emphasizeSelectionCommand,
    fontSizeCommand,
    colorSelectionCommand,
    highlightSelectionCommand
  );

  const compileFileFromExplorerCommand = vscode.commands.registerCommand('contextIntellisense.compileFileFromExplorer', async (targetUri) => {
    await compileActiveOrProvidedDocument(targetUri);
  });
  context.subscriptions.push(compileFileFromExplorerCommand);

  const compileMainCommand = vscode.commands.registerCommand('contextIntellisense.compileMainFile', async () => {
    await compileMainFile();
  });
  context.subscriptions.push(compileMainCommand);

  const showPdfCommand = vscode.commands.registerCommand('contextIntellisense.showPdfForCurrentFile', async () => {
    await showPdfForActiveDocument();
  });
  context.subscriptions.push(showPdfCommand);

  const synctexForwardCommand = vscode.commands.registerCommand('contextIntellisense.synctexForward', async () => {
    if (getSyncTexMode() === 'off') {
      return;
    }
    await forwardSyncTex(vscode.window.activeTextEditor, {
      allowEmpty: true,
      trigger: 'contextMenu'
    });
  });
  context.subscriptions.push(synctexForwardCommand);

  const environmentPairHighlightDecoration = vscode.window.createTextEditorDecorationType({
    backgroundColor: new vscode.ThemeColor('editor.wordHighlightTextBackground'),
    overviewRulerColor: new vscode.ThemeColor('editorOverviewRuler.wordHighlightTextForeground'),
    overviewRulerLane: vscode.OverviewRulerLane.Right
  });
  context.subscriptions.push(environmentPairHighlightDecoration);
  function updateEnvironmentPairHighlights(activeEditor = vscode.window.activeTextEditor) {
    for (const editor of vscode.window.visibleTextEditors) {
      const ranges = editor === activeEditor && isContextTexDocument(editor.document)
        ? environmentPairs.getHighlightRanges(editor.document, editor.selection.active)
        : [];
      editor.setDecorations(environmentPairHighlightDecoration, ranges);
    }
  }
  updateEnvironmentPairHighlights();

  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection((event) => {
    if (event.kind !== vscode.TextEditorSelectionChangeKind.Mouse) {
      return;
    }

    const mode = getSyncTexMode();
    if (mode !== 'doubleclick') {
      return;
    }

    handleDoubleClickSelection(event);
  }));

  const clearWorkspaceCommand = vscode.commands.registerCommand('contextIntellisense.clearWorkspace', async () => {
    await clearWorkspaceTemporaryFiles();
  });
  context.subscriptions.push(clearWorkspaceCommand);

  const hideTemporaryFilesCommand = vscode.commands.registerCommand(
    'contextIntellisense.hideTemporaryFiles',
    () => setTemporaryFilesHidden(true)
  );
  const showTemporaryFilesCommand = vscode.commands.registerCommand(
    'contextIntellisense.showTemporaryFiles',
    () => setTemporaryFilesHidden(false)
  );
  context.subscriptions.push(hideTemporaryFilesCommand, showTemporaryFilesCommand);

  context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(editor => {
    void applyContextLanguageToOpenDocuments().then(() => {
      updateEnvironmentPairHighlights();
      wysiwymDecorations.updateVisibleEditors();
    });
    updateCommandVisibility();
  }));

  const pdfWatcher = vscode.workspace.createFileSystemWatcher('**/*.pdf');
  for (const event of [pdfWatcher.onDidCreate, pdfWatcher.onDidChange, pdfWatcher.onDidDelete]) {
    context.subscriptions.push(event(() => {
      updateCommandVisibility();
      scheduleTemporaryFileDecorationRefresh();
    }));
  }
  context.subscriptions.push(pdfWatcher);

  for (const pattern of temporaryFileWatcherPatterns) {
    const watcher = vscode.workspace.createFileSystemWatcher(pattern);
    for (const event of [watcher.onDidCreate, watcher.onDidChange, watcher.onDidDelete]) {
      context.subscriptions.push(event(scheduleTemporaryFileDecorationRefresh));
    }
    context.subscriptions.push(watcher);
  }
  for (const pattern of workspaceInterfaceWatcherPatterns) {
    const watcher = vscode.workspace.createFileSystemWatcher(pattern);
    for (const event of [watcher.onDidCreate, watcher.onDidChange, watcher.onDidDelete]) {
      context.subscriptions.push(event(() => scheduleWorkspaceArgumentDiagnostics(true)));
    }
    context.subscriptions.push(watcher);
  }
  context.subscriptions.push(vscode.workspace.onDidSaveTextDocument(document => {
    if (document.uri.scheme === 'file' && isConTeXtTexFilePath(document.uri.fsPath)) {
      scheduleWorkspaceArgumentDiagnostics(true);
      if (/^env(?:ironment)?(?:[-_.]|$)/i.test(path.basename(document.uri.fsPath))
        || /\\(?:definehighlight|define(?:global)?color)\b/i.test(document.getText())) {
        void wysiwymDecorations.refresh();
      }
    }
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(
    () => {
      scheduleTemporaryFileDecorationRefresh();
      scheduleWorkspaceArgumentDiagnostics(true);
      void wysiwymDecorations.refresh();
    }
  ));
  for (const event of [
    vscode.workspace.onDidCreateFiles,
    vscode.workspace.onDidDeleteFiles,
    vscode.workspace.onDidRenameFiles
  ]) {
    context.subscriptions.push(event(e => {
      scheduleWorkspaceArgumentDiagnostics(e);
      void wysiwymDecorations.refresh();
    }));
  }
  context.subscriptions.push({ dispose: () => clearTimeout(temporaryFileRefreshTimer) });

  // A PDF that was closed must be allowed to open again. Without this,
  // openedPdfPaths incorrectly treats the closed tab as still open.
  context.subscriptions.push(vscode.window.tabGroups.onDidChangeTabs(forgetClosedPdfTabs));

  void connectAcademicPdfViewerSyncTex();

  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(() => {
    void applyContextLanguageToOpenDocuments();
  }));

  void maybeRunFirstStartSetup(false);

  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration('contextIntellisense.fontSwitches.bold')
      || e.affectsConfiguration('contextIntellisense.fontSwitches.emphasize')) {
      void wysiwymDecorations.refresh();
    }

    if (e.affectsConfiguration('contextIntellisense.distribution.texRootPath')) {
      void refreshWorkspaceArgumentDiagnostics(true);
      refreshDecorations();
      void wysiwymDecorations.refresh();
    }

    if (e.affectsConfiguration('contextIntellisense.compilation.mainFilePath')) {
      refreshDecorations();
      updateCommandVisibility();
    }

    if (e.affectsConfiguration('contextIntellisense.explorer.hideTemporaryFiles')) {
      scheduleTemporaryFileDecorationRefresh();
    }

  }));

  updateCommandVisibility();

  const triggerArgumentContextCommand = vscode.commands.registerCommand(
    'contextIntellisense.triggerArgumentContext',
    () => triggerArgumentInformation(vscode.window.activeTextEditor, true)
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

  const selector = [
    { language: 'context-tex', scheme: 'file' },
    { language: 'context-internal-tex', scheme: 'file' }
  ];
  context.subscriptions.push(vscode.languages.registerCodeActionsProvider(
    selector,
    {
      provideCodeActions(document, range, codeActionContext) {
        return codeActionContext.diagnostics
          .filter(diagnostic => diagnostic.severity === vscode.DiagnosticSeverity.Warning
            && ['ConTeXt', 'ConTeXt IntelliSense'].includes(diagnostic.source))
          .map(diagnostic => {
            const action = new vscode.CodeAction('Ignore this warning', vscode.CodeActionKind.QuickFix);
            action.diagnostics = [diagnostic];
            action.command = {
              title: 'Ignore this warning',
              command: 'contextIntellisense.ignoreWarning',
              arguments: [document.uri.toString(), warningKey(document.uri, diagnostic)]
            };
            return action;
          });
      }
    },
    { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }
  ));
  const semanticLegend = new vscode.SemanticTokensLegend([
    'function',
    'namespace',
    'contextStructure',
    'contextMathEnvironment',
    'contextMathCommand',
    'enumMember',
    'typeParameter',
    'keyword',
    'contextCommand',
    'contextFallbackCommand',
    'contextRegister',
    'contextArgumentKey',
    'contextArgumentValue',
    'contextReference'
  ]);
  context.subscriptions.push(vscode.languages.registerDocumentSemanticTokensProvider(
    selector,
    {
      async provideDocumentSemanticTokens(document) {
        await intellisenseDataLoad;
        await scanWorkspaceReferences();
        const builder = new vscode.SemanticTokensBuilder(semanticLegend);
        getCommandSemanticTokens(document, builder);
        getMacroParameterSemanticTokens(document, builder);
        getArgumentSemanticTokens(document, builder);
        getReferenceSemanticTokens(document, builder);
        return builder.build();
      }
    },
    semanticLegend
  ));

  context.subscriptions.push(vscode.languages.registerFoldingRangeProvider(
    selector,
    { provideFoldingRanges: getStructureFoldingRanges }
  ));

  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider(
    selector,
    { provideDocumentSymbols }
  ));

  context.subscriptions.push(vscode.languages.registerLinkedEditingRangeProvider(
    selector,
    { provideLinkedEditingRanges: environmentPairs.provideLinkedEditingRanges }
  ));

  let forcedUnfilteredArgumentPosition;
  let forcedUnfilteredArgumentTimer;
  /** Opens a clicked argument's completion list only when it has candidates. */
  function triggerArgumentCompletions(editor, position, bracketContext) {
    void createArgumentCompletionItems(bracketContext, true).then(items => {
      if (!items.length || vscode.window.activeTextEditor !== editor
        || !editor.selection.active.isEqual(position)) {
        return;
      }
      forcedUnfilteredArgumentPosition = { uri: editor.document.uri.toString(), position };
      clearTimeout(forcedUnfilteredArgumentTimer);
      forcedUnfilteredArgumentTimer = setTimeout(() => { forcedUnfilteredArgumentPosition = undefined; }, 1000);
      void vscode.commands.executeCommand('editor.action.triggerSuggest');
    });
  }

  const completionTriggerCharacters = [
    '[', ',', ' ', '\\',
    ...'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
  ];
  const provider = vscode.languages.registerCompletionItemProvider(
    selector,
    {
      /** Resolves command, argument, and embedded-Lua suggestions at the cursor. */
      async provideCompletionItems(document, position, completionContext) {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor?.document === document
          && activeEditor.selections.some(selection => !selection.isEmpty)) {
          return undefined;
        }
        const forcedPosition = forcedUnfilteredArgumentPosition;
        const forceAll = forcedPosition?.uri === document.uri.toString()
          && forcedPosition.position.isEqual(position);
        if (forceAll) {
          clearTimeout(forcedUnfilteredArgumentTimer);
          forcedUnfilteredArgumentPosition = undefined;
        }
        const luaItems = await embeddedLua.provideCompletionItems(document, position, completionContext?.triggerCharacter);
        if (luaItems !== undefined) {
          return createDynamicCompletionList(luaItems);
        }
        await intellisenseDataLoad;
        if (getCache().commandCompletions.length === 0) {
          return undefined;
        }

        const line = document.lineAt(position.line).text;
        const linePrefix = line.slice(0, position.character);
        const lineSuffix = line.slice(position.character);

        const commandMatch = linePrefix.match(/\\([A-Za-z]*)$/);
        if (commandMatch) {
          const commandEnd = position.character + (lineSuffix.match(/^[A-Za-z]*/) || [''])[0].length;
          const items = setCompletionRange(
            createCommandItems(commandMatch[1], document.languageId === 'context-internal-tex'),
            position,
            commandMatch.index + 1,
            commandEnd
          );
          return createDynamicCompletionList(
            environmentPairs.prepareCompletionItems(document, position, items)
          );
        }

        const bracketContext = getDocumentBracketContext(document, position);
        if (bracketContext && (bracketContext.isTopLevel || bracketContext.isEnumListValue)) {
          const showAll = forceAll || completionContext?.triggerKind === vscode.CompletionTriggerKind.Invoke;
          const items = await createArgumentCompletionItems(bracketContext, showAll);
          const start = getCompletionRangeStart(linePrefix, position, bracketContext);
          const tokenEnd = getCompletionRangeEnd(lineSuffix, linePrefix, position, bracketContext);
          return createDynamicCompletionList(setCompletionRange(
            items,
            position,
            start,
            tokenEnd,
            {
              keyEndCharacter: tokenEnd + (line[tokenEnd] === '=' ? 1 : 0),
              showAll,
              lineText: line
            }
          ), !showAll);
        }

        return undefined;
      }
    },
    ...completionTriggerCharacters
  );

  context.subscriptions.push(provider);

  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection(event => {
    if (event.textEditor !== vscode.window.activeTextEditor) {
      return;
    }
    updateEnvironmentPairHighlights(event.textEditor);
    scheduleArgumentInformation(
      event.textEditor,
      true,
      true
    );
    if (event.textEditor.selections.some(selection => !selection.isEmpty)) {
      return;
    }
    if (event.kind === vscode.TextEditorSelectionChangeKind.Mouse) {
      const editor = event.textEditor;
      const position = editor.selection.active;
      const bracketContext = getDocumentBracketContext(editor.document, position);
      if (bracketContext && (bracketContext.isTopLevel || bracketContext.isEnumListValue)) {
        const line = editor.document.lineAt(position.line).text;
        const prefix = line.slice(0, position.character);
        const suffix = line.slice(position.character);
        const start = getCompletionRangeStart(prefix, position, bracketContext);
        const end = getCompletionRangeEnd(suffix, prefix, position, bracketContext);
        const tokenCharacter = character => /[\p{L}\p{N}_./:-]/u.test(character || '');
        if ((tokenCharacter(prefix.at(-1)) || tokenCharacter(suffix[0]))
          && (start < position.character || end > position.character)) {
          triggerArgumentCompletions(editor, position, bracketContext);
        }
      }
    }
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    workspaceReferenceState.promise = null;
    inferredReferenceArguments.clear();
    if (isContextTexDocument(event.document)) {
      scheduleWorkspaceArgumentDiagnostics();
    }
    const editor = vscode.window.activeTextEditor;
    for (const visibleEditor of vscode.window.visibleTextEditors) {
      if (visibleEditor.document === event.document) wysiwymDecorations.update(visibleEditor);
    }
    if (editor && editor.document === event.document) {
      updateEnvironmentPairHighlights(editor);
        scheduleArgumentInformation(editor);
      if (event.contentChanges.some(change => change.rangeLength > 0 && !change.text)) {
        setTimeout(() => {
          if (vscode.window.activeTextEditor !== editor) return;
          const position = editor.selection.active;
          const prefix = editor.document.lineAt(position.line).text.slice(0, position.character);
          const commandMatch = prefix.match(/\\([A-Za-z]*)$/);
          if (commandMatch) {
            if (createCommandItems(commandMatch[1], editor.document.languageId === 'context-internal-tex').length) {
              vscode.commands.executeCommand('editor.action.triggerSuggest');
            }
            return;
          }
          const bracketContext = getDocumentBracketContext(editor.document, position);
          if (bracketContext) {
            void createArgumentCompletionItems(bracketContext, true).then(items => {
              if (items.length && vscode.window.activeTextEditor === editor
                && editor.selection.active.isEqual(position)) {
                vscode.commands.executeCommand('editor.action.triggerSuggest');
              }
            });
          }
        }, 0);
      }
    }
  }));

  const signatureProvider = vscode.languages.registerSignatureHelpProvider(
    selector,
    {
      async provideSignatureHelp(document, position, token, helpContext) {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor?.document === document
          && activeEditor.selections.some(selection => !selection.isEmpty)) {
          return undefined;
        }
        const luaHelp = await embeddedLua.provideSignatureHelp(document, position, helpContext?.triggerCharacter);
        if (luaHelp !== undefined) {
          return luaHelp;
        }
        await intellisenseDataLoad;
        const line = document.lineAt(position.line).text;
        const linePrefix = line.slice(0, position.character);
        const bracketContext = getDocumentBracketContext(document, position);
        if (!bracketContext) {
          return null;
        }

        const commandName = bracketContext.commandName;
        const specs = getCommandSignatureSpecs(
          commandName,
          bracketContext.bracketCount,
          bracketContext.argumentDelimiters,
          bracketContext.argumentIndex,
          bracketContext.currentSegment,
          bracketContext.remainingSegment
        )
          .filter(spec => spec.kind !== 'content');
        if (specs.length === 0) {
          return null;
        }

        const activeParameterIndex = Math.min(
          bracketContext.argumentIndex,
          Math.max(specs.length - 1, 0)
        );
        const activeAssignmentKey = bracketContext && bracketContext.commandName === commandName
          ? getActiveAssignmentKey(bracketContext.currentSegment, bracketContext.remainingSegment)
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

  context.subscriptions.push(vscode.languages.registerHoverProvider(selector, {
    provideHover: (document, position) => embeddedLua.provideHover(document, position)
  }));

  const fileDecorationProvider = vscode.window.registerFileDecorationProvider({
    onDidChangeFileDecorations: fileDecorationsEmitter.event,
    provideFileDecoration(uri) {
      if (uri.scheme !== 'file') {
        return undefined;
      }

      if (temporaryFilePaths.has(normalizePathForComparison(uri.fsPath))) {
        return {
          tooltip: 'Clear Workspace can delete this temporary file.',
          color: new vscode.ThemeColor('list.deemphasizedForeground')
        };
      }

      const mainFilePath = getResolvedMainFilePath();
      if (!mainFilePath) {
        return undefined;
      }

      if (normalizePathForComparison(uri.fsPath) !== normalizePathForComparison(mainFilePath)) {
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
  fileDecorationsEmitter.fire();
  refreshTemporaryFileDecorations();
}


  return activate;
};
