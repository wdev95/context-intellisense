/** Creates the XML-driven ConTeXt document-symbol provider. */
module.exports = function createSymbolProvider(deps) {
  const { vscode, getCache, findMatchingDelimiter, getStructureEnvironmentMatches } = deps;

  /** Returns top-level argument contents following a command invocation. */
  function readArguments(text, start) {
    const argumentsFound = [];
    let position = start;
    while (position < text.length) {
      while (/\s/.test(text[position] || '')) {
        position += 1;
      }
      const open = text[position];
      const close = open === '[' ? ']' : open === '{' ? '}' : '';
      if (!close) {
        break;
      }
      const end = findMatchingDelimiter(text, position, open, close);
      argumentsFound.push({ open, content: text.slice(position + 1, end) });
      position = end < text.length ? end + 1 : text.length;
    }
    return argumentsFound;
  }

  /** Splits an argument into top-level comma-separated assignments. */
  function splitAssignments(content) {
    const parts = [];
    let start = 0;
    let depth = 0;
    for (let index = 0; index <= content.length; index += 1) {
      const character = content[index];
      if (character === '{' || character === '[') {
        depth += 1;
      } else if (character === '}' || character === ']') {
        depth = Math.max(0, depth - 1);
      } else if ((character === ',' || index === content.length) && depth === 0) {
        parts.push(content.slice(start, index).trim());
        start = index + 1;
      }
    }
    return parts;
  }

  /** Returns the heading text from an invocation's arguments. */
  function getHeadingText(argumentsFound) {
    for (const argument of argumentsFound) {
      for (const assignment of splitAssignments(argument.content)) {
        const match = assignment.match(/^title\s*=\s*([\s\S]*)$/i);
        if (match) {
          return cleanTitle(match[1]);
        }
      }
    }
    const first = argumentsFound.find(argument => argument.open === '{');
    return first ? cleanTitle(first.content) : '';
  }

  /** Normalizes a source title for display in the Outline view. */
  function cleanTitle(value) {
    const text = String(value || '').trim();
    if (text.startsWith('{') && text.endsWith('}')) {
      return cleanTitle(text.slice(1, -1));
    }
    return text.replace(/\s+/g, ' ') || '';
  }

  /** Returns whether XML metadata describes a sectioning command. */
  function isSectionCommand(metadata) {
    return metadata
      && metadata.category === 'structure'
      && metadata.file === 'strc-sec.mkxl'
      && (Number.isInteger(metadata.structureRank) || metadata.type === 'environment');
  }

  function getSectionMetadata(commandName, cache) {
    const metadata = cache.commandMetadata.get(commandName);
    return isSectionCommand(metadata) ? metadata : null;
  }

  /** Builds nested Outline symbols from sectioning commands in source order. */
  function provideDocumentSymbols(document) {
    const cache = getCache();
    const text = document.getText();
    const symbols = [];
    const matches = getStructureEnvironmentMatches(
      document,
      commandName => Boolean(getSectionMetadata(commandName, cache)),
      commandName => getSectionMetadata(commandName, cache)?.structureRank
    );
    for (const match of matches) {
      const commandName = match.command;
      const argumentsFound = readArguments(text, match.startEnd);
      const title = getHeadingText(argumentsFound) || commandName;
      const metadata = getSectionMetadata(commandName, cache);
      if (!metadata) {
        continue;
      }
      symbols.push({
        rank: metadata.structureRank ?? 0,
        symbol: new vscode.DocumentSymbol(
          title,
          metadata.structureName || commandName.replace(/^start/, ''),
          vscode.SymbolKind.Class,
          new vscode.Range(document.positionAt(match.start), document.positionAt(match.end ?? text.length)),
          new vscode.Range(document.positionAt(match.start), document.positionAt(match.startEnd))
        )
      });
    }

    const roots = [];
    const stack = [];
    for (const entry of symbols) {
      while (stack.length > 0 && stack[stack.length - 1].rank >= entry.rank) {
        stack.pop();
      }
      if (stack.length === 0) {
        roots.push(entry.symbol);
      } else {
        stack[stack.length - 1].symbol.children.push(entry.symbol);
      }
      stack.push(entry);
    }
    return roots;
  }

  return { provideDocumentSymbols };
};
