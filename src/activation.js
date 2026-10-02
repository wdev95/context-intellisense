/** Creates the VS Code activation entry point from explicit services. */
const createSyncTexActivationServices = require('./activation-synctex');

module.exports = function createActivator(deps) {
  const { isExistingFile, isConTeXtTexFilePath, toWorkspaceRelativePath, resolveConfiguredPath, normalizePathForComparison, inferredReferenceArguments, workspaceReferenceState } = deps;
  const { setDataWarningReporter } = deps;
  const { getMacroParameterSemanticTokens, getStructureFoldingRanges, getCommandSemanticTokens, getArgumentSemanticTokens, getReferenceSemanticTokens, provideDocumentSymbols } = deps;
  const { vscode, fs, path, zlib, isContextTexDocument, applyContextLanguageToOpenDocuments, findTexRootPathOnPath, collectWorkspaceTemporaryFiles, collectWorkspaceTemporaryFileState, temporaryFileExcludePatterns, temporaryFileWatcherPatterns, getAcademicPdfViewerExtension, formatSyncTexTracePath, formatSyncTexTraceUri, findOpenPdfGroup, forgetClosedPdfTabs, runProcess, resolveSyncTexExecutable, resolveMtxRunExecutable, resolveXmlPathFromTexRoot, resolveContextExecutableFromTexRoot, resolveMainFilePath, openPdfFile, reloadActiveAcademicPdf, loadData, buildSignatureParts, getAcademicPdfViewerApi, parseSyncTexViewResult, parseSyncTexEditResult, parseMtxSyncTexFindResult, parseMtxSyncTexReportResult, triggerArgumentInformation, scheduleArgumentInformation, scanWorkspaceReferences, scanWorkspaceArgumentDiagnostics, initializeWorkspaceFileCache, createEmbeddedLuaBridge, getCommandSignatureSpecs, getActiveAssignmentKey, createCommandItems, createArgumentCompletionItems, getDocumentBracketContext, setCompletionRange, createDynamicCompletionList, getCompletionRangeStart, getCache } = deps;
function activate(context) {
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
  const trackedArgumentDiagnostics = new Map();
  let argumentDiagnosticsRefreshTimer;
  let argumentDiagnosticsRefreshId = 0;
  const compileStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  compileStatus.name = 'ConTeXt compilation';
  compileStatus.hide();
  context.subscriptions.push(outputChannel, syncTexOutputChannel, embeddedLua, fileDecorationsEmitter, codeLensesEmitter, compileDiagnostics, argumentDiagnostics, compileStatus);

  /** Starts one shared load for all IntelliSense providers. */
  function loadIntellisenseData(texRootPath = getConfiguredTexRootPath()) {
    intellisenseDataLoad = loadData(texRootPath);
    return intellisenseDataLoad;
  }

  /** Replaces diagnostics from the workspace argument validator. */
  function setArgumentDiagnostics(uri, diagnostics) {
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
      await (reloadIntellisenseData ? loadIntellisenseData() : intellisenseDataLoad);
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
  function scheduleWorkspaceArgumentDiagnostics() {
    clearTimeout(argumentDiagnosticsRefreshTimer);
    argumentDiagnosticsRefreshTimer = setTimeout(() => {
      void refreshWorkspaceArgumentDiagnostics();
    }, 300);
  }

  context.subscriptions.push({
    dispose: () => {
      clearTimeout(argumentDiagnosticsRefreshTimer);
      argumentDiagnosticsRefreshId += 1;
    }
  });

  /** Writes a complete JSON diagnostic record for one SyncTeX operation. */
  const syncTex = createSyncTexActivationServices({ context, outputChannel: syncTexOutputChannel, vscode, fs, path, zlib, isContextTexDocument, isExistingFile, normalizePathForComparison, toWorkspaceRelativePath, resolveConfiguredPath, resolveXmlPathFromTexRoot, resolveMainFilePath, resolveContextExecutableFromTexRoot, resolveSyncTexExecutable, resolveMtxRunExecutable, runProcess, parseSyncTexViewResult, parseSyncTexEditResult, parseMtxSyncTexFindResult, parseMtxSyncTexReportResult, getAcademicPdfViewerApi, findOpenPdfGroup, formatSyncTexTracePath, formatSyncTexTraceUri });
const { writeSyncTexTrace, getWorkspaceRootPath, getSyncTexMode, getSyncTexIntegrationMode, getConfiguredTexRootPath, getConfiguredMainFilePath, resolveWorkspaceRelativePath, resolveViewerPdfPath, hasConfiguredTexRoot, getResolvedTexRootPath, getResolvedXmlPath, getResolvedContextExecutable, getResolvedMainFilePath, findSyncTexSidecar, ensureGzippedSyncTexSidecar, configureAcademicPdfViewerBridge, resolveSyncTexInputName, getSyncTexWorkingDirectory, toSyncTexRelativePath, resolveSyncTexSourcePath, resolveSyncTexPdfPath, buildSyncTexCommand, forwardSyncTex, findVisibleSourceEditor, findOpenSourceTab, findNormalEditorColumn, showSyncTexSource, handleInverseSyncTex, connectAcademicPdfViewerSyncTex } = syncTex;

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
      if (vscode.workspace.getConfiguration('contextIntellisense').get('hideTemporaryFiles', false)) {
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
    await config.update('texRootPath', texRootPath, vscode.ConfigurationTarget.Global);
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
    await config.update('mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
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
        await config.update('texRootPath', detectedTexRootPath, vscode.ConfigurationTarget.Global);
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
    compileDiagnostics.set(uri, [diagnostic]);
  }

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
    compileStatus.text = '$(sync~spin) ConTeXt: compiling';
    compileStatus.tooltip = `Compiling ${path.basename(resolvedTargetPath)}`;
    compileStatus.show();
    outputChannel.clear();
    const compileArgs = ['--synctex', resolvedTargetPath];
    let compilationFailure = null;
    let compilationWarning = '';
    let pdfOpenWarning = '';
    beginCompileFileDecorationBatch();
    try {
      await vscode.window.withProgress({
        location: vscode.ProgressLocation.Window,
        title,
        cancellable: false
      }, async () => {
        compileStatus.text = '$(sync~spin) ConTeXt: compiling';
        compileStatus.tooltip = `Compiling ${path.basename(resolvedTargetPath)}`;
        const result = await runProcess(contextExecutable, compileArgs, cwd, outputChannel);
        const processOutput = [
          result.stdout || '',
          result.stderr || '',
          readTextIfExists(contextLogPath),
          readTextIfExists(errorLogPath)
        ].join('\n');
        if (hasPdfLockMessage(processOutput)) {
          compilationWarning = `ConTeXt could not write the output PDF. It may be locked by an external viewer. Close it in the viewer and compile again: ${pdfPath}`;
          publishCompileWarning(resolvedTargetPath, compilationWarning);
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
          compilationWarning = `ConTeXt did not produce an output PDF: ${pdfPath}`;
          publishCompileWarning(resolvedTargetPath, compilationWarning);
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
          .get('openPdfAfterCompile', true);
        if (openPdfAfterCompile) {
          try {
            await openPdfFile(pdfPath);
          } catch (error) {
            pdfOpenWarning = `Compilation succeeded, but the PDF could not be opened: ${error.message || error}`;
            publishCompileWarning(resolvedTargetPath, pdfOpenWarning);
          }
        }
      });
    } finally {
      endCompileFileDecorationBatch();
      compileStatus.hide();
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
        'hideTemporaryFiles',
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
  void applyContextLanguageToOpenDocuments();
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
    await config.update('mainFilePath', mainFilePath, vscode.ConfigurationTarget.Workspace);
    refreshDecorations();
    vscode.window.setStatusBarMessage(`ConTeXt IntelliSense main file set to: ${mainFilePath}`, 4000);
  });
  context.subscriptions.push(setMainFromExplorerCommand);

  const compileCurrentCommand = vscode.commands.registerCommand('contextIntellisense.compileCurrentFile', async (targetUri) => {
    await compileActiveOrProvidedDocument(targetUri);
  });
  context.subscriptions.push(compileCurrentCommand);

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

  const hideTemporaryFilesCommand = vscode.commands.registerCommand(
    'contextIntellisense.hideTemporaryFiles',
    () => setTemporaryFilesHidden(true)
  );
  const showTemporaryFilesCommand = vscode.commands.registerCommand(
    'contextIntellisense.showTemporaryFiles',
    () => setTemporaryFilesHidden(false)
  );
  context.subscriptions.push(hideTemporaryFilesCommand, showTemporaryFilesCommand);

  context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(() => {
    void applyContextLanguageToOpenDocuments();
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
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(
    () => {
      scheduleTemporaryFileDecorationRefresh();
      scheduleWorkspaceArgumentDiagnostics();
    }
  ));
  for (const event of [
    vscode.workspace.onDidCreateFiles,
    vscode.workspace.onDidDeleteFiles,
    vscode.workspace.onDidRenameFiles
  ]) {
    context.subscriptions.push(event(scheduleWorkspaceArgumentDiagnostics));
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
    if (e.affectsConfiguration('contextIntellisense.texRootPath')) {
      void refreshWorkspaceArgumentDiagnostics(true);
      refreshDecorations();
    }

    if (e.affectsConfiguration('contextIntellisense.mainFilePath')) {
      refreshDecorations();
      updateCommandVisibility();
    }

    if (e.affectsConfiguration('contextIntellisense.hideTemporaryFiles')) {
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

  const completionTriggerCharacters = [
    '[', ',', '=', ' ', '\\',
    ...'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
  ];
  const provider = vscode.languages.registerCompletionItemProvider(
    selector,
    {
      async provideCompletionItems(document, position, completionContext) {
        const luaItems = await embeddedLua.provideCompletionItems(document, position, completionContext?.triggerCharacter);
        if (luaItems !== undefined) {
          return createDynamicCompletionList(luaItems);
        }
        await intellisenseDataLoad;
        if (getCache().commandCompletions.length === 0) {
          return [];
        }

        const line = document.lineAt(position.line).text;
        const linePrefix = line.slice(0, position.character);
        const lineSuffix = line.slice(position.character);

        const commandMatch = linePrefix.match(/\\([A-Za-z]*)$/);
        if (commandMatch) {
          return createDynamicCompletionList(setCompletionRange(
            createCommandItems(commandMatch[1], document.languageId === 'context-internal-tex'),
            position,
            commandMatch.index + 1
          ));
        }

        const bracketContext = getDocumentBracketContext(document, position);
        if (bracketContext && (bracketContext.isTopLevel || bracketContext.isEnumListValue)) {
          const items = await createArgumentCompletionItems(bracketContext);
          return createDynamicCompletionList(setCompletionRange(
            items,
            position,
            getCompletionRangeStart(linePrefix, position, bracketContext)
          ));
        }

        return [];
      }
    },
    ...completionTriggerCharacters
  );

  context.subscriptions.push(provider);

  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection(event => {
    if (event.textEditor !== vscode.window.activeTextEditor
      || (event.kind !== vscode.TextEditorSelectionChangeKind.Keyboard
        && event.kind !== vscode.TextEditorSelectionChangeKind.Mouse)) {
      return;
    }
    scheduleArgumentInformation(
      event.textEditor,
      true,
      true
    );
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    workspaceReferenceState.promise = null;
    inferredReferenceArguments.clear();
    if (isContextTexDocument(event.document)) {
      scheduleWorkspaceArgumentDiagnostics();
    }
    const editor = vscode.window.activeTextEditor;
    if (editor && editor.document === event.document) {
      scheduleArgumentInformation(editor);
    }
  }));

  const signatureProvider = vscode.languages.registerSignatureHelpProvider(
    selector,
    {
      async provideSignatureHelp(document, position, token, helpContext) {
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
          bracketContext.currentSegment
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
