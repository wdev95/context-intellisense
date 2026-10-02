const fs = require('fs');
const path = require('path');

const temporaryFileExtensions = new Set(['.log', '.tuc', '.pgf', '.synctex']);

const externalFigureExtensions = new Set([
  'pdf', 'png', 'jpg', 'jpeg', 'jbig', 'jb2', 'jp2', 'jpx',
  'mps', 'svg', 'eps', 'gif', 'tif', 'tiff', 'webp', 'u3d', 'swf'
]);

/** Returns whether a path is a supported ConTeXt external-figure file. */
function isExternalFigureFile(filePath) {
  return externalFigureExtensions.has(path.extname(filePath).slice(1).toLowerCase());
}

/** Identifies generated ConTeXt images that should be treated as temporary. */
function isGeneratedImageFile(filePath) {
  return /^m_k_i_v/i.test(path.basename(filePath)) && isExternalFigureFile(filePath);
}

/** Identifies temporary ConTeXt outputs so inventory updates can ignore them. */
function isTemporaryWorkspaceFile(filePath, siblingFiles) {
  const extension = path.extname(filePath).toLowerCase();
  const basename = path.basename(filePath).toLowerCase();
  const texSibling = `${path.basename(filePath, path.extname(filePath))}.tex`.toLowerCase();
  const hasTexSibling = extension === '.pdf' && (siblingFiles
    ? siblingFiles.has(texSibling)
    : fs.existsSync(path.join(path.dirname(filePath), texSibling)));
  return temporaryFileExtensions.has(extension)
    || basename.endsWith('.synctex.gz')
    || isGeneratedImageFile(filePath)
    || (extension === '.pdf' && hasTexSibling);
}

/** Returns the watcher glob for files used by workspace completions. */
function workspaceFileWatcherPattern() {
  return `**/*.{${[...externalFigureExtensions, 'tex'].join(',')}}`;
}

/** Returns Explorer exclusion globs for generated image files. */
function generatedImagePatterns() {
  const extensions = [...externalFigureExtensions].join(',');
  return [`**/m_k_i_v*.{${extensions}}`];
}

module.exports = {
  temporaryFileExtensions,
  isExternalFigureFile,
  isGeneratedImageFile,
  isTemporaryWorkspaceFile,
  workspaceFileWatcherPattern,
  generatedImagePatterns
};
