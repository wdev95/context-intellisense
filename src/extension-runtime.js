const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const zlib = require('zlib');
const vscode = require('vscode');

const {
  createDataCache,
  uniqueValues,
  normalizeTypeLabel,
  getDelimiterInfo
} = require('./core/data-cache');
const {
  describeArgumentSpec,
  buildArgumentSnippet,
  makeCommandInsertText,
  makeArgumentPreview
} = require('./core/command-signatures');
const { parseAttributes, ensureEntry, getReferenceNames } = require('./core/xml-helpers');
const { loadInterfaceXml, expandInterfaceDefinitions } = require('./core/interface-xml');
const {
  isExistingDirectory,
  isExistingFile,
  resolveConfiguredPath,
  normalizePathForComparison,
  isConTeXtTexFilePath,
  toWorkspaceRelativePath
} = require('./core/path-utils');
const {
  parseSyncTexViewResult,
  parseSyncTexEditResult,
  parseMtxSyncTexFindResult,
  parseMtxSyncTexReportResult
} = require('./core/synctex-parser');
const { getCommandSemanticType, getInternalSemanticType } = require('./core/semantic-types');
const { stripTeXComment } = require('./core/tex-text');
const createSemanticProviders = require('./providers-semantic');
const createSymbolProvider = require('./providers-symbols');
const createPathResolvers = require('./providers-paths');
const createCompletionPresentation = require('./providers-completion-presentation');
const createCommandXmlParser = require('./providers-command-xml');
const createDataProviders = require('./providers-data');
const createArgumentProviders = require('./providers-arguments');
const createEmbeddedLuaBridge = require('./providers-embedded-lua');
const createSystemProviders = require('./providers-system');
const createActivator = require('./activation');

let cache = createDataCache();

const workspaceReferenceState = { promise: null };
const workspaceFileState = { promise: null };
const inferredReferenceArguments = new Set();

const REFERENCE_TOKEN_PATTERN = /\b[A-Za-z][A-Za-z0-9_-]*:[A-Za-z0-9][A-Za-z0-9._-]*/g;

const CONTEXT_TEX_LANGUAGE_IDS = new Set(['context-tex', 'context-internal-tex']);

/** Returns whether a document uses a ConTeXt TeX language mode. */
function isContextTexDocument(document) {
  return Boolean(document && CONTEXT_TEX_LANGUAGE_IDS.has(document.languageId));
}

// Keep track of PDFs opened by this extension. This also covers a PDF editor
// detached into another VS Code window, which is not visible in this window's
// tab groups.
const openedPdfPaths = new Set();
const ACADEMIC_PDF_VIEWER_EXTENSION_ID = 'ovolab-veritas.academic-pdf-viewer';
const ACADEMIC_PDF_VIEW_TYPE = 'academicPdfViewer.pdf';
const ACADEMIC_PDF_RELOAD_COMMAND = 'academicPdfViewer.reload';

let argumentProviders;
const completionPresentation = createCompletionPresentation({
  vscode,
  describeArgumentSpec,
  getDelimiterInfo,
  normalizeTypeLabel,
  getCommandSemanticType,
  getCommandEntry: (...args) => dataProviders.getCommandEntry(...args),
  getArgumentParameterNames: (...args) => argumentProviders.getArgumentParameterNames(...args),
  getArgumentParameterValues: (...args) => argumentProviders.getArgumentParameterValues(...args),
  getArgumentParameterTypes: (...args) => argumentProviders.getArgumentParameterTypes(...args),
  uniqueValues
});
const { escapeHtmlText, buildHtmlKeyValueTable, formatValueCell, chunkValuesForDisplay, collectOrderedValues, buildSignatureParts, getCommandCompletionKind } = completionPresentation;

const { parseCommandMap } = createCommandXmlParser({ fs, path, vscode, parseAttributes, ensureEntry, getReferenceNames, makeCommandInsertText, buildArgumentSnippet, getCommandCompletionKind });

let pathResolvers;
const dataProviders = createDataProviders({
  fs,
  path,
  vscode,
  loadInterfaceXml,
  expandInterfaceDefinitions,
  createDataCache,
  parseCommandMap,
  getCommandCompletionKind,
  stripTeXComment,
  getCache: () => cache,
  resolveConfiguredPath,
  resolveXmlPathFromTexRoot: (...args) => pathResolvers.resolveXmlPathFromTexRoot(...args),
  resolveContextExecutableFromTexRoot: (...args) => pathResolvers.resolveContextExecutableFromTexRoot(...args)
});
const { cleanFontToken, parseFontSourceFile, loadFontCompletions, getCommandEntry, loadMathPrimitiveMetadata, getContextSourceFiles, loadRegisterMetadata, createRegisterCompletions, addFontMetadata, loadData, setDataWarningReporter } = dataProviders;

argumentProviders = createArgumentProviders({ vscode, fs, path, isContextTexDocument, isConTeXtTexFilePath, inferredReferenceArguments, REFERENCE_TOKEN_PATTERN, workspaceReferenceState, workspaceFileState, getCache: () => dataProviders.getCache(), getCommandEntry, buildSignatureParts, makeArgumentPreview, uniqueValues });
const { isEscaped, findMatchingDelimiter, mapCommandArguments, getBracketInvocationContext, getDocumentBracketContext, getTopLevelArgumentSegments, findAssignmentSeparator, getArgumentValueSpan, maskTeXComments, getCurrentAssignmentValue, setCompletionRange, createDynamicCompletionList, getCompletionRangeStart, triggerArgumentInformation, scheduleArgumentInformation, getCommandArgumentSpec, getCommandSignatureSpecs, getArgumentParameterNames, getArgumentParameterValues, getArgumentParameterTypes, isReferenceType, scanWorkspaceReferences, scanWorkspaceArgumentDiagnostics, createArgumentCompletionItems, initializeWorkspaceFileCache, getActiveAssignmentKey, createCommandItems } = argumentProviders;

async function applyContextLanguageToOpenDocuments() {
  const updates = [];
  for (const document of vscode.workspace.textDocuments) {
    if (!document || document.uri.scheme !== 'file') {
      continue;
    }

    if (!isConTeXtTexFilePath(document.uri.fsPath)) {
      continue;
    }

    if (isContextTexDocument(document)) {
      continue;
    }

    updates.push(vscode.languages.setTextDocumentLanguage(document, 'context-tex'));
  }

  if (updates.length > 0) {
    await Promise.allSettled(updates);
  }
}

const systemProviders = createSystemProviders({ vscode, fs, path, cp, isExistingDirectory, isExistingFile, resolveConfiguredPath, normalizePathForComparison, toWorkspaceRelativePath, openedPdfPaths, ACADEMIC_PDF_VIEWER_EXTENSION_ID, ACADEMIC_PDF_VIEW_TYPE, ACADEMIC_PDF_RELOAD_COMMAND });
const { collectWorkspaceTemporaryFiles, collectWorkspaceTemporaryFileState, temporaryFileExcludePatterns, temporaryFileWatcherPatterns, searchForFile, getAcademicPdfViewerExtension, formatSyncTexTracePath, formatSyncTexTraceUri, findOpenPdfGroup, forgetClosedPdfTabs, runProcess, resolveSyncTexExecutable, resolveMtxRunExecutable, getAcademicPdfViewerApi, openPdfFile, reloadActiveAcademicPdf } = systemProviders;

pathResolvers = createPathResolvers({ fs, path, isExistingDirectory, isExistingFile, resolveConfiguredPath, searchForFile });
const { resolveXmlPathFromTexRoot, resolveContextExecutableFromTexRoot, resolveMainFilePath } = pathResolvers;

const semanticProviders = createSemanticProviders({
  vscode,
  isEscaped,
  findMatchingDelimiter,
  mapCommandArguments,
  getBracketInvocationContext,
  getDocumentBracketContext,
  getTopLevelArgumentSegments,
  findAssignmentSeparator,
  getArgumentValueSpan,
  maskTeXComments,
  getCommandArgumentSpec,
  getCommandSignatureSpecs,
  getArgumentParameterNames,
  getArgumentParameterTypes,
  getArgumentParameterValues,
  isReferenceType,
  inferredReferenceArguments,
  getCache: () => dataProviders.getCache(),
  getCommandEntry,
  getCommandSemanticType,
  getInternalSemanticType
});
const { getMacroParameterSemanticTokens, getStructureFoldingRanges, getCommandSemanticTokens, getArgumentSemanticTokens, getReferenceSemanticTokens } = semanticProviders;
const { provideDocumentSymbols } = createSymbolProvider({
  vscode,
  getCache: () => dataProviders.getCache(),
  findMatchingDelimiter,
  getStructureEnvironmentMatches: require('./structure-ranges')
});

const activate = createActivator({
  isExistingFile, isConTeXtTexFilePath, toWorkspaceRelativePath, resolveConfiguredPath, normalizePathForComparison, inferredReferenceArguments, workspaceReferenceState,
  vscode, fs, path, zlib, isContextTexDocument, applyContextLanguageToOpenDocuments,
  collectWorkspaceTemporaryFiles, collectWorkspaceTemporaryFileState,
  temporaryFileExcludePatterns, temporaryFileWatcherPatterns, getAcademicPdfViewerExtension,
  formatSyncTexTracePath, formatSyncTexTraceUri, findOpenPdfGroup, forgetClosedPdfTabs,
  runProcess, resolveSyncTexExecutable, resolveMtxRunExecutable, getAcademicPdfViewerApi,
  openPdfFile, reloadActiveAcademicPdf, ...pathResolvers, ...semanticProviders, provideDocumentSymbols,
  loadData, setDataWarningReporter, buildSignatureParts,
  triggerArgumentInformation, scheduleArgumentInformation, scanWorkspaceReferences,
  scanWorkspaceArgumentDiagnostics,
  initializeWorkspaceFileCache,
  createEmbeddedLuaBridge,
  getCommandSignatureSpecs, getActiveAssignmentKey, createCommandItems, createArgumentCompletionItems,
  getDocumentBracketContext, setCompletionRange, createDynamicCompletionList,
  getCompletionRangeStart, parseSyncTexViewResult, parseSyncTexEditResult,
  parseMtxSyncTexFindResult, parseMtxSyncTexReportResult,
  getCache: () => dataProviders.getCache()
});

function deactivate() {
  argumentProviders.dispose();
}

module.exports = {
  activate,
  deactivate
};
