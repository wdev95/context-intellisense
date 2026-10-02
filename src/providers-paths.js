/** Creates TeX path resolvers from filesystem dependencies. */
module.exports = function createPathResolvers(deps) {
  const { fs, path, isExistingDirectory, isExistingFile, resolveConfiguredPath, searchForFile } = deps;

function hasTexRootMarker(texRootPath) {
  return [
    path.join(texRootPath, 'texmf-context'),
    path.join(texRootPath, 'tex'),
    path.join(texRootPath, 'share', 'texmf-context')
  ].some(isExistingDirectory);
}

/** Reads platform folder names from the installed ConTeXt installer metadata. */
function readPlatformFolderNames(texRootPath) {
  const metadataPaths = [
    path.join(texRootPath, 'texmf-context', 'scripts', 'context', 'lua', 'mtx-install.lua'),
    path.join(texRootPath, 'scripts', 'context', 'lua', 'mtx-install.lua')
  ];

  for (const metadataPath of metadataPaths) {
    if (!isExistingFile(metadataPath)) {
      continue;
    }

    try {
      const source = fs.readFileSync(metadataPath, 'utf8');
      const table = source.match(/local\s+platforms\s*=\s*\{([\s\S]*?)\n\s*\}/)?.[1] || '';
      const names = [...table.matchAll(/^\s*\[\s*["']([^"']+)["']\s*\]\s*=\s*["']([^"']+)["']/gm)]
        .map((match) => `texmf-${match[2]}`);
      return [...new Set(names)];
    } catch (error) {
      return [];
    }
  }

  return [];
}

/** Returns whether a binary directory contains a ConTeXt launcher. */
function hasContextExecutable(binaryPath) {
  return ['context', 'context.exe', 'context.cmd', 'context.bat']
    .some((name) => isExistingFile(path.join(binaryPath, name)));
}

/** Finds a ConTeXt distribution root by locating its executable on PATH. */
function findTexRootPathOnPath() {
  const executableNames = process.platform === 'win32'
    ? ['context.exe', 'context.cmd', 'context.bat', 'context']
    : ['context'];
  const pathEntries = String(process.env.PATH || '').split(path.delimiter);

  for (const rawEntry of pathEntries) {
    const entry = rawEntry.trim().replace(/^['"]|['"]$/g, '');
    if (!entry) {
      continue;
    }

    for (const executableName of executableNames) {
      const executablePath = path.join(entry, executableName);
      if (!isExistingFile(executablePath)) {
        continue;
      }

      let candidate = path.dirname(path.dirname(executablePath));
      for (let depth = 0; depth < 8; depth += 1) {
        if (hasTexRootMarker(candidate) && resolveXmlPathFromTexRoot(candidate)) {
          return candidate;
        }

        const parent = path.dirname(candidate);
        if (parent === candidate) {
          break;
        }
        candidate = parent;
      }
    }
  }

  return '';
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

  for (const folderName of readPlatformFolderNames(root)) {
    const binaryPath = path.join(root, folderName, 'bin');
    if (isExistingDirectory(binaryPath) && hasContextExecutable(binaryPath)) {
      return path.join(binaryPath, 'context');
    }
  }

  return 'context';
}

function resolveMainFilePath(configuredMainFilePath, workspaceFolderPath = '') {
  const resolved = resolveConfiguredPath(configuredMainFilePath, workspaceFolderPath);
  return isExistingFile(resolved) ? resolved : '';
}



  return { findTexRootPathOnPath, resolveXmlPathFromTexRoot, resolveContextExecutableFromTexRoot, resolveMainFilePath };
};
