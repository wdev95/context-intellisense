/** Creates the SyncTeX bridge and source-navigation services. */
module.exports = function createSyncTexActivationServices(deps) {
  const { context, outputChannel, vscode, fs, path, zlib, isContextTexDocument, isExistingFile, normalizePathForComparison, toWorkspaceRelativePath, resolveConfiguredPath, resolveXmlPathFromTexRoot, resolveMainFilePath, resolveContextExecutableFromTexRoot, resolveSyncTexExecutable, resolveMtxRunExecutable, runProcess, parseSyncTexViewResult, parseSyncTexEditResult, parseMtxSyncTexFindResult, parseMtxSyncTexReportResult, getAcademicPdfViewerApi, findOpenPdfGroup, formatSyncTexTracePath, formatSyncTexTraceUri } = deps;
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
    return resolveConfiguredPath(getConfiguredTexRootPath());
  }

  function getResolvedXmlPath() {
    const texRootPath = getResolvedTexRootPath();
    return resolveXmlPathFromTexRoot(texRootPath);
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

    const normalizedSourcePath = normalizePathForComparison(sourcePath);
    for (const line of content.split(/\r?\n/)) {
      const match = /^Input:\d+:(.+)$/i.exec(line.trim());
      if (!match) {
        continue;
      }
      const inputName = match[1].trim();
      const inputPath = path.isAbsolute(inputName)
        ? inputName
        : path.resolve(path.dirname(pdfPath), inputName);
      if (normalizePathForComparison(inputPath) === normalizedSourcePath) {
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
    if (!isContextTexDocument(document)) {
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
    const normalizedSourcePath = normalizePathForComparison(sourcePath);
    return vscode.window.visibleTextEditors.find((editor) => {
      return editor.document.uri.scheme === 'file'
        && normalizePathForComparison(editor.document.uri.fsPath) === normalizedSourcePath;
    }) || null;
  }

  /** Finds an already open source tab and returns its editor column. */
  function findOpenSourceTab(sourcePath) {
    const normalizedSourcePath = normalizePathForComparison(sourcePath);
    for (const group of vscode.window.tabGroups.all) {
      for (const tab of group.tabs) {
        const uri = tab.input && tab.input.uri;
        if (uri && uri.scheme === 'file'
          && normalizePathForComparison(uri.fsPath) === normalizedSourcePath) {
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

  
  return { writeSyncTexTrace, getWorkspaceRootPath, getSyncTexMode, getSyncTexIntegrationMode, getConfiguredTexRootPath, getConfiguredMainFilePath, resolveWorkspaceRelativePath, resolveViewerPdfPath, hasConfiguredTexRoot, getResolvedTexRootPath, getResolvedXmlPath, getResolvedContextExecutable, getResolvedMainFilePath, findSyncTexSidecar, ensureGzippedSyncTexSidecar, configureAcademicPdfViewerBridge, resolveSyncTexInputName, getSyncTexWorkingDirectory, toSyncTexRelativePath, resolveSyncTexSourcePath, resolveSyncTexPdfPath, buildSyncTexCommand, forwardSyncTex, findVisibleSourceEditor, findOpenSourceTab, findNormalEditorColumn, showSyncTexSource, handleInverseSyncTex, connectAcademicPdfViewerSyncTex };
};
