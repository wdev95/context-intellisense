const {
  temporaryFileExtensions,
  isTemporaryWorkspaceFile,
  generatedImagePatterns
} = require('./core/workspace-files');

/** Creates filesystem, PDF-viewer, and process services. */
module.exports = function createSystemProviders(deps) {
  const { vscode, fs, path, cp, isExistingDirectory, isExistingFile, resolveConfiguredPath, normalizePathForComparison, toWorkspaceRelativePath, openedPdfPaths, ACADEMIC_PDF_VIEWER_EXTENSION_ID, ACADEMIC_PDF_VIEW_TYPE, ACADEMIC_PDF_RELOAD_COMMAND } = deps;
const temporaryFileExcludePatterns = [
  ...[...temporaryFileExtensions].map(extension => `**/*${extension}`),
  '**/*.synctex.gz',
  ...generatedImagePatterns()
];
const temporaryFileWatcherPatterns = [
  `**/*.{${[...temporaryFileExtensions].map(extension => extension.slice(1)).concat('tex').join(',')}}`,
  '**/*.synctex.gz',
  ...generatedImagePatterns()
];

function collectWorkspaceTemporaryFileState(workspaceFolders) {
  const files = new Set();
  const expectedPdfPaths = new Set();

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
        if (extension === '.tex') {
          expectedPdfPaths.add(
            path.join(folderPath, `${entry.name.slice(0, -extension.length)}.pdf`)
          );
        }
        if (isTemporaryWorkspaceFile(fullPath, fileNames)) {
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

  return {
    files: Array.from(files).sort((a, b) => a.localeCompare(b)),
    expectedPdfPaths: Array.from(expectedPdfPaths).sort((a, b) => a.localeCompare(b))
  };
}

function collectWorkspaceTemporaryFiles(workspaceFolders) {
  return collectWorkspaceTemporaryFileState(workspaceFolders).files;
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

function getAcademicPdfViewerExtension() {
  return vscode.extensions.getExtension(ACADEMIC_PDF_VIEWER_EXTENSION_ID);
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
  const normalizedPdfPath = normalizePathForComparison(pdfPath);
  for (const group of vscode.window.tabGroups.all) {
    if (group.tabs.some((tab) => {
      const uri = tab.input && tab.input.uri;
      return uri && uri.scheme === 'file' && normalizePathForComparison(uri.fsPath) === normalizedPdfPath;
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
      openedPdfPaths.delete(normalizePathForComparison(uri.fsPath));
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

/** Opens a local file through the operating system's registered handler. */
function openWithSystemDefault(filePath) {
  const absolutePath = path.resolve(filePath);
  let command;
  let args;

  if (process.platform === 'win32') {
    const encodedPath = Buffer.from(absolutePath, 'utf8').toString('base64');
    const script = `$ErrorActionPreference='Stop'; $pdf=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${encodedPath}')); Start-Process -FilePath $pdf`;
    command = process.env.SystemRoot
      ? path.join(process.env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe')
      : 'powershell.exe';
    args = ['-NoLogo', '-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')];
  } else if (process.platform === 'darwin') {
    command = 'open';
    args = [absolutePath];
  } else {
    command = 'xdg-open';
    args = [absolutePath];
  }

  return new Promise((resolve, reject) => {
    cp.execFile(command, args, { windowsHide: true }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(String(stderr || error.message || error).trim()));
        return;
      }
      resolve(true);
    });
  });
}

async function openPdfFile(pdfPath, options = {}) {
  const normalizedPdfPath = normalizePathForComparison(pdfPath);
  const existingGroup = findOpenPdfGroup(pdfPath);
  const forceFocus = options.forceFocus === true;

  // Reopening the custom editor recreates the PDF viewer and loses its
  // split, detached-window, position, and size state.
  if (openedPdfPaths.has(normalizedPdfPath) && !existingGroup && !forceFocus) {
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
    await openWithSystemDefault(pdfPath);
    // Do not cache system-opened PDFs: calling the OS handler after a later
    // compile is how external viewers receive/reload the freshly built file.
    return true;
  }
  openedPdfPaths.add(normalizedPdfPath);
  return true;
}

async function reloadActiveAcademicPdf() {
  const extension = getAcademicPdfViewerExtension();
  if (!extension) {
    return false;
  }

  try {
    if (!extension.isActive) {
      await extension.activate();
    }
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

  return { collectWorkspaceTemporaryFiles, collectWorkspaceTemporaryFileState, temporaryFileExcludePatterns, temporaryFileWatcherPatterns, searchForFile, getAcademicPdfViewerExtension, formatSyncTexTracePath, formatSyncTexTraceUri, findOpenPdfGroup, forgetClosedPdfTabs, getInstalledPdfViewers, isWindowsBatch, runProcess, resolveSyncTexExecutable, resolveMtxRunExecutable, getAcademicPdfViewerApi, openPdfFile, reloadActiveAcademicPdf };
};
