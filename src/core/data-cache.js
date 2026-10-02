/** Creates the empty cache used before and between XML loads. */
function createDataCache(xmlPath = '') {
  return {
    xmlPath,
    commandMap: new Map(),
    commandCompletions: [],
    commandArgumentSpecs: new Map(),
    commandArgumentSpecVariants: new Map(),
    structureEnvironments: new Set(),
    commandMetadata: new Map(),
    parameterValueDefaults: new Map()
  };
}

/** Returns unique truthy values while preserving their input order. */
function uniqueValues(values) {
  return Array.from(new Set(values.filter(Boolean)));
}

/** Normalizes an XML type label for display. */
function normalizeTypeLabel(value) {
  const raw = String(value || '').trim();
  if (!raw) {
    return '';
  }
  return (raw.startsWith('cd:') ? raw.slice(3) : raw).toUpperCase();
}

/** Returns the delimiters represented by an XML argument kind. */
function getDelimiterInfo(delimiter) {
  switch (delimiter) {
    case 'brace':
      return { open: '{', close: '}' };
    case 'paren':
      return { open: '(', close: ')' };
    case 'none':
      return { open: ' ', close: '' };
    default:
      return { open: '[', close: ']' };
  }
}

module.exports = {
  createDataCache,
  uniqueValues,
  normalizeTypeLabel,
  getDelimiterInfo
};
