/** Creates semantic-token and folding providers from explicit dependencies. */
module.exports = function createSemanticProviders(deps) {
  const {
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
    getCache,
    getCommandEntry,
    getCommandSemanticType,
    getInternalSemanticType
  } = deps;
  const getStructureEnvironmentMatches = require('./structure-ranges');
  function stripTeXComment(line) {
  for (let index = 0; index < line.length; index += 1) {
    if (line[index] !== '%' || (index > 0 && line[index - 1] === '\\')) {
      continue;
    }
    return line.slice(0, index);
  }
  return line;
}

/** Returns whether a source offset is inside an unescaped TeX comment. */
function isTeXCommentOffset(text, offset) {
  const lineStart = text.lastIndexOf('\n', offset - 1) + 1;
  for (let index = lineStart; index < offset; index += 1) {
    if (text[index] !== '%' || isEscaped(text, index)) {
      continue;
    }
    return true;
  }
  return false;
}

/** Finds the outer body of a TeX macro definition after its signature. */
function findMacroBodyStart(text, start) {
  let bracketDepth = 0;
  for (let index = start; index < text.length; index += 1) {
    if (isEscaped(text, index)) {
      continue;
    }
    if (text[index] === '[') {
      bracketDepth += 1;
    } else if (text[index] === ']') {
      bracketDepth = Math.max(0, bracketDepth - 1);
    } else if (text[index] === '{' && bracketDepth === 0) {
      return index;
    }
  }
  return -1;
}

/** Adds semantic tokens for parameters declared by TeX-style macro definitions. */
function getMacroParameterSemanticTokens(document, tokenBuilder) {
  const text = document.getText();
  const definitionRe = /\\(?:(?:protected|global|outer|long|unexpanded)\s+)*(?:def|gdef|edef|xdef|define[A-Za-z]+|install[A-Za-z]+)(?![A-Za-z])/gi;
  const parameterRe = /#+([0-9]+)/g;
  let definition;
  while ((definition = definitionRe.exec(text)) !== null) {
    if (isTeXCommentOffset(text, definition.index)) {
      continue;
    }
    const bodyStart = findMacroBodyStart(text, definition.index + definition[0].length);
    if (bodyStart < 0) {
      continue;
    }
    const declared = new Set();
    parameterRe.lastIndex = definition.index;
    let parameter;
    while ((parameter = parameterRe.exec(text)) !== null && parameter.index < bodyStart) {
      declared.add(parameter[1]);
    }
    if (declared.size === 0) {
      continue;
    }
    const bodyEnd = findMatchingDelimiter(text, bodyStart, '{', '}');
    const end = bodyEnd < text.length ? bodyEnd + 1 : text.length;
    parameterRe.lastIndex = definition.index;
    while ((parameter = parameterRe.exec(text)) !== null && parameter.index < end) {
      if (parameter.index >= bodyStart && !declared.has(parameter[1])) {
        continue;
      }
      if (isTeXCommentOffset(text, parameter.index)) {
        continue;
      }
      tokenBuilder.push(
        new vscode.Range(document.positionAt(parameter.index), document.positionAt(parameter.index + parameter[0].length)),
        'contextArgumentKey'
      );
    }
  }
}

/** Returns folding ranges for XML-defined structure environments. */
function getStructureFoldingRanges(document) {
  return getStructureEnvironmentMatches(
    document,
    command => getCache().structureEnvironments.has(command),
    command => getCache().commandMetadata.get(command)?.structureRank
  )
    .filter(match => match.startLine < match.endLine)
    .map(match => new vscode.FoldingRange(match.startLine, match.endLine, vscode.FoldingRangeKind.Region));
}

/** Returns whether the entered invocation selects a reference-valued signature. */
function isReferenceInvocation(commandName, argumentDelimiters) {
  if (!argumentDelimiters || argumentDelimiters.length === 0) {
    return false;
  }
  const bracketCount = argumentDelimiters.filter(delimiter => delimiter === 'bracket').length;
  const specs = getCommandSignatureSpecs(commandName, bracketCount, argumentDelimiters);
  return specs.some(spec => (spec.keywordTypes || []).some(isReferenceType));
}

/** Returns whether any XML signature for a command accepts a reference. */
function commandHasReferenceArgument(commandName) {
  const variants = getCache().commandArgumentSpecVariants.get(commandName) || [];
  return variants.some(specs => specs.some(spec =>
    (spec.keywordTypes || []).some(isReferenceType)
  ));
}

/** Builds semantic tokens for commands defined by the active XML tree. */
function getCommandSemanticTokens(document, tokenBuilder) {
  const internalDocument = document.languageId === 'context-internal-tex';
  const commandRe = internalDocument
    ? /\\([A-Za-z@:_!?]+)/g
    : /\\([A-Za-z]+)/g;
  for (let lineNumber = 0; lineNumber < document.lineCount; lineNumber += 1) {
    commandRe.lastIndex = 0;
    const line = stripTeXComment(document.lineAt(lineNumber).text);
    const argumentsByOpen = mapCommandArguments(line);
    let match;
    while ((match = commandRe.exec(line)) !== null) {
      const metadata = getCache().commandMetadata.get(match[1])
        || getCache().commandMetadata.get(match[1].match(/^[A-Za-z]+/)?.[0]);
      const argumentDelimiters = [...argumentsByOpen.values()]
        .filter(argument => argument.commandIndex === match.index)
        .sort((left, right) => left.argumentIndex - right.argumentIndex)
        .map(argument => argument.delimiter);
      const referenceInvocation = argumentDelimiters.length > 0
        && (isReferenceInvocation(match[1], argumentDelimiters)
          || commandHasReferenceArgument(match[1]));
      const tokenType = internalDocument
        ? getInternalSemanticType(match[1], metadata)
        : metadata?.category === 'mathematics' && referenceInvocation
          ? 'function'
        : getCommandSemanticType(metadata);
      if (tokenType) {
        tokenBuilder.push(
          new vscode.Range(
            lineNumber,
            match.index,
            lineNumber,
            match.index + match[0].length
          ),
          tokenType
        );
      }
    }
  }
}

/** Adds semantic tokens for known assignment keys and their constrained values. */
function getArgumentSemanticTokens(document, tokenBuilder) {
  const text = document.getText();
  const source = maskTeXComments(text);
  const argumentsByCommand = new Map();
  for (const [open, argument] of mapCommandArguments(source)) {
    const argumentsForCommand = argumentsByCommand.get(argument.commandIndex) || [];
    argumentsForCommand.push({ open, ...argument });
    argumentsByCommand.set(argument.commandIndex, argumentsForCommand);
  }

  const tokens = [];
  for (const argumentsForCommand of argumentsByCommand.values()) {
    argumentsForCommand.sort((left, right) => left.argumentIndex - right.argumentIndex);
    const first = argumentsForCommand[0];
    const delimiters = argumentsForCommand.map(argument => argument.delimiter);
    const bracketCount = delimiters.filter(delimiter => delimiter === 'bracket').length;
    for (const argument of argumentsForCommand) {
      if (argument.delimiter !== 'bracket') {
        continue;
      }
      const spec = getCommandArgumentSpec(
        first.commandName,
        argument.argumentIndex,
        bracketCount,
        delimiters,
        source.slice(argument.open + 1, argument.close)
      );
      if (!spec || spec.kind !== 'assignments') {
        continue;
      }

      const entry = getCommandEntry(first.commandName);
      const contentStart = argument.open + 1;
      const content = source.slice(contentStart, argument.close);
      for (const segment of getTopLevelArgumentSegments(content)) {
        const equalsIndex = findAssignmentSeparator(segment.text);
        if (equalsIndex < 0) {
          continue;
        }
        const key = segment.text.slice(0, equalsIndex).trim();
        if (!key) {
          continue;
        }
        const values = getArgumentParameterValues(spec, entry, key);
        const types = getArgumentParameterTypes(spec, entry, key);
        const knownKey = getArgumentParameterNames(spec).some(name => name.toLowerCase() === key.toLowerCase())
          || values.size > 0
          || types.size > 0;
        if (!knownKey) {
          continue;
        }

        const keyStart = contentStart + segment.start + segment.text.indexOf(key);
        tokens.push({ start: keyStart, end: keyStart + key.length, type: 'contextArgumentKey' });
        if (values.size === 0 || [...types].some(isReferenceType)) {
          continue;
        }

        const span = getArgumentValueSpan(
          segment.text.slice(equalsIndex + 1),
          contentStart + segment.start + equalsIndex + 1
        );
        const members = span.grouped
          ? getTopLevelArgumentSegments(span.value)
          : [{ text: span.value, start: 0 }];
        for (const member of members) {
          const memberSpan = getArgumentValueSpan(member.text, span.start + member.start);
          if ([...values].some(value => value.toLowerCase() === memberSpan.value.toLowerCase())) {
            tokens.push({
              start: memberSpan.start,
              end: memberSpan.end,
              type: 'contextArgumentValue'
            });
          }
        }
      }
    }
  }

  tokens.sort((left, right) => left.start - right.start);
  for (const token of tokens) {
    tokenBuilder.push(
      new vscode.Range(document.positionAt(token.start), document.positionAt(token.end)),
      token.type
    );
  }
}

/** Adds semantic tokens for reference and bibliography arguments. */
function getReferenceSemanticTokens(document, tokenBuilder) {
  const commandRe = /\\([A-Za-z]+)/g;
  for (let lineNumber = 0; lineNumber < document.lineCount; lineNumber += 1) {
    const line = stripTeXComment(document.lineAt(lineNumber).text);
    commandRe.lastIndex = 0;
    let commandMatch;
    while ((commandMatch = commandRe.exec(line)) !== null) {
      let position = commandMatch.index + commandMatch[0].length;
      let argumentIndex = 0;
      let bracketCount = 0;
      const argumentDelimiters = [];
      while (position < line.length) {
        while (/\s/.test(line[position] || '')) {
          position += 1;
        }
        const open = line[position];
        if (open !== '[' && open !== '{') {
          break;
        }
        const close = findMatchingDelimiter(line, position, open, open === '[' ? ']' : '}');
        if (open === '[') {
          bracketCount += 1;
        }
        argumentDelimiters.push(open === '[' ? 'bracket' : 'brace');
        const content = line.slice(position + 1, close);
        const spec = getCommandArgumentSpec(
          commandMatch[1],
          argumentIndex,
          bracketCount,
          argumentDelimiters,
          content
        );
        const argumentSegments = getTopLevelArgumentSegments(content);
        const referenceAssignments = argumentSegments.filter(segment => {
          const equals = findAssignmentSeparator(segment.text);
          return equals >= 0 && /^reference$/i.test(segment.text.slice(0, equals).trim());
        });
        const referenceArgument = inferredReferenceArguments.has(`${commandMatch[1]}:${argumentIndex}`)
          || spec && (
          spec.kind === 'keywords' && (spec.keywordTypes || []).some(isReferenceType)
          ) || referenceAssignments.length > 0
          || (commandHasReferenceArgument(commandMatch[1]) && /[A-Za-z0-9]+:[A-Za-z0-9._-]+/.test(content));
        if (referenceArgument) {
          const entry = getCommandEntry(commandMatch[1]);
          const segments = spec?.kind === 'assignments' || referenceAssignments.length > 0
            ? argumentSegments.flatMap(segment => {
              const equals = findAssignmentSeparator(segment.text);
              if (equals < 0) {
                return [];
              }
              const parameterName = segment.text.slice(0, equals).trim();
              if (!/^reference$/i.test(parameterName)
                && ![...getArgumentParameterTypes(spec, entry, parameterName)].some(isReferenceType)) {
                return [];
              }
              const value = segment.text.slice(equals + 1);
              const leadingWhitespace = value.search(/\S|$/);
              return [{
                text: value.slice(leadingWhitespace),
                start: segment.start + equals + 1 + leadingWhitespace
              }];
            })
            : [{ text: content, start: 0 }];
          for (const segment of segments) {
            const referenceRe = /[A-Za-z0-9][A-Za-z0-9:._-]*/g;
            let reference;
            while ((reference = referenceRe.exec(segment.text)) !== null) {
              const start = position + 1 + segment.start + reference.index;
              tokenBuilder.push(
                new vscode.Range(lineNumber, start, lineNumber, start + reference[0].length),
                'contextReference'
              );
            }
          }
        }
        argumentIndex += 1;
        position = close < line.length ? close + 1 : line.length;
      }
    }
  }
}

  return {
    stripTeXComment,
    isTeXCommentOffset,
    findMacroBodyStart,
    getMacroParameterSemanticTokens,
    getStructureFoldingRanges,
    isReferenceInvocation,
    commandHasReferenceArgument,
    getCommandSemanticTokens,
    getArgumentSemanticTokens,
    getReferenceSemanticTokens
  };
};
