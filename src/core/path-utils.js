const fs = require('fs');
const path = require('path');

/** Returns whether a path points to an existing directory. */
function isExistingDirectory(folderPath) {
  return Boolean(folderPath && fs.existsSync(folderPath) && fs.statSync(folderPath).isDirectory());
}

/** Returns whether a path points to an existing file. */
function isExistingFile(filePath) {
  return Boolean(filePath && fs.existsSync(filePath) && fs.statSync(filePath).isFile());
}

/** Resolves an absolute or workspace-relative configuration path. */
function resolveConfiguredPath(inputPath, workspaceFolderPath = '') {
  const value = String(inputPath || '').trim();
  if (!value) {
    return '';
  }
  return path.isAbsolute(value)
    ? value
    : path.resolve(workspaceFolderPath || '', value);
}

/** Normalizes a path using the current platform's case rules. */
function normalizePathForComparison(filePath) {
  const value = String(filePath || '');
  if (!value) {
    return '';
  }
  const normalized = path.normalize(path.resolve(value));
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

/** Returns whether a path uses a supported ConTeXt TeX extension. */
function isConTeXtTexFilePath(filePath) {
  return ['.tex', '.mkiv', '.mkxl', '.mkvi', '.mkil', '.mkix', '.mkxi', '.mklx']
    .includes(path.extname(String(filePath || '')).toLowerCase());
}

/** Converts an in-workspace path to a portable relative path. */
function toWorkspaceRelativePath(filePath, workspaceFolderPath) {
  const relativePath = path.relative(path.resolve(workspaceFolderPath), path.resolve(filePath));
  if (!relativePath || relativePath.startsWith(`..${path.sep}`) || path.isAbsolute(relativePath)) {
    return '';
  }
  return relativePath.split(path.sep).join('/');
}

module.exports = {
  isExistingDirectory,
  isExistingFile,
  resolveConfiguredPath,
  normalizePathForComparison,
  isConTeXtTexFilePath,
  toWorkspaceRelativePath
};
