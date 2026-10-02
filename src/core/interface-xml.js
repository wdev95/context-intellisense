const fs = require('fs');
const path = require('path');
const { parseAttributes } = require('./xml-helpers');

/** Loads an interface tree while preserving referenced and commented definitions. */
function loadInterfaceXml(xmlPath, visited = new Set()) {
  const absolutePath = path.resolve(xmlPath);
  if (visited.has(absolutePath) || !fs.existsSync(absolutePath)) {
    return '';
  }
  visited.add(absolutePath);
  const text = fs.readFileSync(absolutePath, 'utf8');
  const directory = path.dirname(absolutePath);
  const references = [...text.matchAll(/<cd:interfacefile\b[^>]*\bfilename="([^"]+)"[^>]*\/?>/g)];
  return text + references
    .map(([, filename]) => loadInterfaceXml(path.join(directory, filename), visited))
    .join('\n');
}

/** Expands XML definitions referenced through cd:resolve elements. */
function expandInterfaceDefinitions(xmlText) {
  const definitions = new Map();
  const defineRe = /<cd:define\b([^>]*)>([\s\S]*?)<\/cd:define>/g;
  let match;
  while ((match = defineRe.exec(xmlText)) !== null) {
    const name = parseAttributes(match[1]).name;
    if (name) {
      definitions.set(name, match[2]);
    }
  }

  function expand(fragment, stack = new Set()) {
    return fragment.replace(/<cd:resolve\b([^>]*?)\s*\/?>/g, (reference, attributes) => {
      const name = parseAttributes(attributes).name;
      const definition = definitions.get(name);
      if (!definition || stack.has(name)) {
        return reference;
      }
      const nextStack = new Set(stack);
      nextStack.add(name);
      return expand(definition, nextStack);
    });
  }

  return expand(xmlText);
}

module.exports = { loadInterfaceXml, expandInterfaceDefinitions };
