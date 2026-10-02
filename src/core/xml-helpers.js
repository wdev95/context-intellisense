/** Parses the attributes of a ConTeXt interface XML tag. */
function parseAttributes(tag) {
  const attributes = {};
  const attributeRe = /(\w+)\s*=\s*"([^"]*)"/g;
  let match;
  while ((match = attributeRe.exec(tag)) !== null) {
    attributes[match[1]] = match[2];
  }
  return attributes;
}

/** Returns the mutable command entry for an XML command name. */
function ensureEntry(map, name) {
  if (!map.has(name)) {
    map.set(name, {
      keywords: new Set(),
      parameters: new Set(),
      parameterValues: new Map(),
      parameterTypes: new Map(),
      parameterValueDefaults: new Map(),
      inherits: new Set(),
      parameterSources: new Map()
    });
  }
  return map.get(name);
}

/** Returns names referenced by XML inheritance or resolution elements. */
function getReferenceNames(xmlFragment) {
  const names = [];
  const referenceRe = /<cd:(?:inherit|resolve)\b([^>]*)\/>/g;
  let match;
  while ((match = referenceRe.exec(xmlFragment)) !== null) {
    const name = parseAttributes(match[1]).name?.trim();
    if (name) {
      names.push(name);
    }
  }
  return names;
}

module.exports = { parseAttributes, ensureEntry, getReferenceNames };
