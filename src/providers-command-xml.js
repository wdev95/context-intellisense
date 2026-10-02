/** Parses the ConTeXt command XML into the shared command model. */
module.exports = function createCommandXmlParser(deps) {
  const { fs, path, vscode, parseAttributes, ensureEntry, getReferenceNames, makeCommandInsertText, buildArgumentSnippet, getCommandCompletionKind } = deps;

function parseCommandMap(xmlText) {
  const commandMap = new Map();
  const completionCandidates = new Map();
  const commandArgumentSpecCandidates = new Map();
  const structureEnvironments = new Set();
  const commandMetadata = new Map();

  function registerCommandMetadata(commandName, attrs) {
    if (!commandName) {
      return;
    }
    const categoryPriority = {
      mathematics: 4,
      fonts: 3,
      structure: 2
    };
    const previous = commandMetadata.get(commandName) || {};
    const category = String(attrs.category || '').toLowerCase();
    const previousCategory = String(previous.category || '').toLowerCase();
    commandMetadata.set(commandName, {
      category: (categoryPriority[category] || 0) >= (categoryPriority[previousCategory] || 0)
        ? category
        : previousCategory,
      file: attrs.file || previous.file || '',
      level: attrs.level === 'primitive' || previous.level === 'primitive'
        ? 'primitive'
        : attrs.level || previous.level || '',
      type: attrs.type || previous.type || '',
      referenceSourceArgument: /^\d+$/.test(String(attrs.referenceSourceArgument ?? ''))
        ? Number(attrs.referenceSourceArgument)
        : previous.referenceSourceArgument,
      structureName: attrs.structureName || previous.structureName || '',
      structureRank: Number.isInteger(attrs.structureRank)
        ? attrs.structureRank
        : previous.structureRank
    });
  }

  function parseArgumentSpecs(commandBlock) {
    const out = [];
    const argumentsBlockMatch = commandBlock.match(/<cd:arguments\b[^>]*>([\s\S]*?)<\/cd:arguments>/);
    if (!argumentsBlockMatch) {
      return out;
    }

    const argumentsBlock = argumentsBlockMatch[1];
    const argBlockRe = /<cd:(keywords|assignments|content|csname)\b([^>]*?)(?:\/\>|>([\s\S]*?)<\/cd:\1>)/g;
    let am;
    while ((am = argBlockRe.exec(argumentsBlock)) !== null) {
      const kind = am[1];
      const attrs = parseAttributes(am[2]);
      const body = am[3] || '';
      const rawDelimiter = (attrs.delimiters || '').trim().toLowerCase();
      const delimiter = rawDelimiter === 'braces'
        ? 'brace'
        : rawDelimiter === 'parenthesis'
          ? 'paren'
          : rawDelimiter === 'none'
            ? 'none'
            : 'bracket';
      const optional = (attrs.optional || '').trim().toLowerCase() === 'yes';
      const list = (attrs.list || '').trim().toLowerCase() === 'yes';

      if (kind === 'keywords') {
        const keywordValues = [];
        const keywordTypes = [];
        const cRe = /<cd:constant\b([^>]*)\/>/g;
        let cm;
        while ((cm = cRe.exec(body)) !== null) {
          const cAttrs = parseAttributes(cm[1]);
          const explicitValue = (cAttrs.value || '').trim();
          const explicitType = (cAttrs.type || '').trim();

          if (explicitValue && !explicitValue.startsWith('cd:')) {
            keywordValues.push(explicitValue);
          }

          if (explicitType) {
            keywordTypes.push(explicitType);
          }
        }

        out.push({ kind, optional, delimiter, list, keywordValues, keywordTypes });
        continue;
      }

      if (kind === 'assignments') {
        const parameterNames = [];
        const parameterTypes = new Map();
        const parameterValues = new Map();
        const parameterSources = getReferenceNames(body);
        let hasParameterTags = false;
        let allowsArbitraryKeys = false;
        const parameterRe = /<cd:parameter\b([^>]*?)(?:\/\>|>([\s\S]*?)<\/cd:parameter>)/g;
        let pm;
        while ((pm = parameterRe.exec(body)) !== null) {
          hasParameterTags = true;
          const pAttrs = parseAttributes(pm[1]);
          const parameterName = (pAttrs.name || '').trim();
          if (parameterName === 'cd:key') {
            allowsArbitraryKeys = true;
          }
          if (parameterName && parameterName !== 'cd:key') {
            parameterNames.push(parameterName);
            if (!parameterTypes.has(parameterName)) {
              parameterTypes.set(parameterName, []);
            }
            if (!parameterValues.has(parameterName)) {
              parameterValues.set(parameterName, []);
            }
          }

          const parameterBody = pm[2] || '';
          const cRe = /<cd:constant\b([^>]*)\/>/g;
          let cm;
          while ((cm = cRe.exec(parameterBody)) !== null) {
            const cAttrs = parseAttributes(cm[1]);
            const explicitType = (cAttrs.type || '').trim();
            if (explicitType && parameterName && parameterName !== 'cd:key') {
              parameterTypes.get(parameterName).push(explicitType);
            }
            const value = (cAttrs.value || cAttrs.type || '').trim();
            if (value && !value.startsWith('cd:') && parameterName && parameterName !== 'cd:key') {
              parameterValues.get(parameterName).push(value);
            }
          }
        }

        out.push({ kind, optional, delimiter, list, parameterNames, parameterTypes, parameterValues, parameterSources, hasParameterTags, allowsArbitraryKeys });
        continue;
      }

      out.push({ kind, optional, delimiter, list });
    }

    return out;
  }

  /** Expands an XML sequence into the generated command name for one instance. */
  function getGeneratedCommandName(commandBlock, instanceName) {
    const sequence = commandBlock.match(/<cd:sequence\b[^>]*>([\s\S]*?)<\/cd:sequence>/);
    if (!sequence) {
      return instanceName;
    }

    const parts = [];
    const partRe = /<cd:(string|instance)\b([^>]*)\/>/g;
    let part;
    while ((part = partRe.exec(sequence[1])) !== null) {
      const attrs = parseAttributes(part[2]);
      parts.push(part[1] === 'instance' ? instanceName : attrs.value || '');
    }
    return parts.join('') || instanceName;
  }

  function getRequiredBracketArgumentCount(commandBlock) {
    const argumentsBlockMatch = commandBlock.match(/<cd:arguments\b[^>]*>([\s\S]*?)<\/cd:arguments>/);
    if (!argumentsBlockMatch) {
      return 0;
    }

    const argumentsBlock = argumentsBlockMatch[1];
    const argStartRe = /<cd:(keywords|assignments)\b([^>]*)>/g;
    let count = 0;
    let match;

    while ((match = argStartRe.exec(argumentsBlock)) !== null) {
      const attrs = parseAttributes(match[2]);
      const optional = (attrs.optional || '').trim().toLowerCase();
      if (optional === 'yes') {
        continue;
      }

      const delimiter = (attrs.delimiters || '').trim().toLowerCase();
      if (delimiter === 'none' || delimiter === 'braces' || delimiter === 'parenthesis') {
        continue;
      }

      // ConTeXt defaults to square brackets when no explicit delimiter is given.
      count++;
    }

    return count;
  }

  function registerCompletion(label, item, meta = {}) {
    if (!completionCandidates.has(label)) {
      completionCandidates.set(label, []);
    }

    completionCandidates.get(label).push({
      kind: item.kind,
      insertText: item.insertText,
      sortWeight: item.sortWeight,
      meta
    });
  }

  function registerCommandArgumentSpecs(commandName, argumentSpecs, variant) {
    if (!commandName || !Array.isArray(argumentSpecs)) {
      return;
    }
    if (!commandArgumentSpecCandidates.has(commandName)) {
      commandArgumentSpecCandidates.set(commandName, []);
    }
    commandArgumentSpecCandidates.get(commandName).push({
      argumentSpecs,
      variant: (variant || '').toLowerCase()
    });
  }

  function getInsertTextValue(candidate) {
    if (!candidate || candidate.insertText === undefined || candidate.insertText === null) {
      return '';
    }

    if (typeof candidate.insertText === 'string') {
      return candidate.insertText;
    }

    if (typeof candidate.insertText.value === 'string') {
      return candidate.insertText.value;
    }

    return String(candidate.insertText);
  }

  function scoreCompletionCandidate(label, candidate) {
    const text = getInsertTextValue(candidate);
    const meta = candidate.meta || {};
    const normalizedVariant = String(meta.variant || '').toLowerCase();
    const requiredBracketCount = Number(meta.requiredBracketCount || 0);

    let score = 0;

    // Prefer canonical (non-string) XML variants.
    if (normalizedVariant === 'string') {
      score -= 200;
    } else if (normalizedVariant) {
      score += 20;
    } else {
      score += 140;
    }

    // Prefer environment-aware start/stop snippets where relevant.
    if (meta.type === 'environment') {
      score += 80;
      if (label.startsWith('\\start')) {
        score += 40;
      }
    }

    // Mandatory [] arguments are most important for this workflow.
    score += requiredBracketCount * 120;
    if (text.includes('[') && text.includes(']')) {
      score += 40;
    }
    if (/\$\{\d+/.test(text)) {
      score += 20;
    }

    if (typeof candidate.insertText !== 'string' && typeof candidate.insertText?.value === 'string') {
      score += 30;
    }

    // Stable tie-breaker.
    score += Math.min(text.length, 100) / 1000;
    return score;
  }

  function makeEnvironmentInsertText(startName, argumentSpecs) {
    const stopName = startName.startsWith('start') && startName.length > 5
      ? `stop${startName.slice(5)}`
      : `stop${startName}`;

    const argumentSnippet = buildArgumentSnippet(argumentSpecs, 1);
    return new vscode.SnippetString(`${startName}${argumentSnippet.text}\n\t$0\n\\${stopName}`);
  }

  /** Returns valid positional forms for the command's optional arguments. */
  function getArgumentSpecVariants(argumentSpecs) {
    if (argumentSpecs.length === 0) {
      return [[]];
    }

    const variants = argumentSpecs.every(spec => spec.optional) ? [[]] : [];
    const collect = (index, selected) => {
      if (index >= argumentSpecs.length) {
        if (selected.length > 0) {
          variants.push(selected);
        }
        return;
      }

      const spec = argumentSpecs[index];
      if (spec.optional) {
        collect(index + 1, [...selected, spec]);
        collect(index + 1, selected);
      } else {
        collect(index + 1, [...selected, spec]);
      }
    };
    collect(0, []);

    const validVariants = variants.filter(variant => {
      const last = variant[variant.length - 1];
      if (!last) {
        return true;
      }
      const lastIndex = argumentSpecs.indexOf(last);
      const hasSkippedAssignment = argumentSpecs
        .slice(lastIndex + 1)
        .some(spec => spec.optional && spec.kind === 'assignments');
      return !(last.optional && last.kind === 'keywords' && hasSkippedAssignment);
    });

    return [...new Map(validVariants.map(variant => {
      const shape = JSON.stringify(variant, (_, value) =>
        value instanceof Map || value instanceof Set ? [...value] : value
      );
      return [shape, variant];
    })).values()];
  }

  const commandRe = /<cd:command\b[^>]*\/>|<cd:command\b[^>]*>[\s\S]*?<\/cd:command>/g;
  const commandBlocks = xmlText.match(commandRe) || [];

  for (const block of commandBlocks) {
    const openTag = block.match(/^<cd:command\b[^>]*\/?>/);
    if (!openTag) {
      continue;
    }

    const attrs = parseAttributes(openTag[0]);
    const name = attrs.name;
    const type = attrs.type || '';
    const category = attrs.category || '';
    const isSectionDefinition = category.toLowerCase() === 'structure'
      && attrs.file === 'strc-sec.mkxl';
    const variant = attrs.variant || '';
    const requiredBracketCount = getRequiredBracketArgumentCount(block);
    const argumentSpecs = parseArgumentSpecs(block);
    if (!name) {
      continue;
    }

    const entry = ensureEntry(commandMap, name);

    const instances = [];
    const instancesBlock = block.match(/<cd:instances\b[^>]*>[\s\S]*?<\/cd:instances>/);
    if (instancesBlock) {
      const iRe = /<cd:constant\b([^>]*)\/>/g;
      let im;
      while ((im = iRe.exec(instancesBlock[0])) !== null) {
        const iAttrs = parseAttributes(im[1]);
        const value = (iAttrs.value || '').trim();
        if (value) {
          instances.push(value);
        }
      }
    }

    if (type === 'environment') {
      if (instances.length > 0) {
        for (const [structureRank, instance] of instances.entries()) {
          const generatedName = getGeneratedCommandName(block, instance);
          const startName = `start${generatedName}`;
          if (isSectionDefinition) {
            structureEnvironments.add(startName);
          }
          const metadata = {
            ...attrs,
            ...(isSectionDefinition && { structureRank }),
            structureName: generatedName
          };
          registerCommandMetadata(startName, metadata);
          registerCommandMetadata(`stop${generatedName}`, metadata);
          const label = `\\${startName}`;
          const item = {
            kind: getCommandCompletionKind(attrs),
            insertText: makeEnvironmentInsertText(startName, argumentSpecs),
            sortWeight: '00'
          };

          commandMap.set(startName, entry);
          registerCompletion(label, item, {
            category,
            type,
            variant,
            requiredBracketCount,
            argumentSpecs,
            environmentStartName: startName
          });
          registerCommandArgumentSpecs(startName, argumentSpecs, variant);
        }
      } else {
        const startName = name.startsWith('start') ? name : `start${name}`;
        if (isSectionDefinition) {
          structureEnvironments.add(startName);
        }
        const structureName = startName.replace(/^start/, '');
        registerCommandMetadata(startName, { ...attrs, structureName });
        registerCommandMetadata(`stop${structureName}`, { ...attrs, structureName });
        const label = `\\${startName}`;
        const item = {
          kind: getCommandCompletionKind(attrs),
          insertText: makeEnvironmentInsertText(startName, argumentSpecs),
          sortWeight: '00'
        };

        commandMap.set(startName, entry);
        registerCompletion(label, item, {
          category,
          type,
          variant,
          requiredBracketCount,
          argumentSpecs,
          environmentStartName: startName
        });
        registerCommandArgumentSpecs(startName, argumentSpecs, variant);
      }
    } else {
      registerCommandMetadata(name, attrs);
      const insertText = makeCommandInsertText(name, argumentSpecs);

      registerCompletion(`\\${name}`, {
        kind: getCommandCompletionKind(attrs),
        insertText,
        sortWeight: '10'
      }, {
        category,
        type,
        variant,
        requiredBracketCount,
        argumentSpecs,
        commandName: name
      });
      registerCommandArgumentSpecs(name, argumentSpecs, variant);

      for (const [structureRank, instance] of instances.entries()) {
        const generatedName = getGeneratedCommandName(block, instance);
        registerCommandMetadata(generatedName, {
          ...attrs,
          ...(isSectionDefinition && { structureRank }),
          structureName: generatedName
        });
        const instanceInsertText = makeCommandInsertText(generatedName, argumentSpecs);

        registerCompletion(`\\${generatedName}`, {
          kind: getCommandCompletionKind(attrs),
          insertText: instanceInsertText,
          sortWeight: '11'
        }, {
          category,
          type,
          variant,
          requiredBracketCount,
          argumentSpecs,
          commandName: generatedName
        });
        registerCommandArgumentSpecs(generatedName, argumentSpecs, variant);
      }
    }

    const keywordBlocks = block.match(/<cd:keywords\b[^>]*>[\s\S]*?<\/cd:keywords>/g) || [];
    for (const keywordBlock of keywordBlocks) {
      const cRe = /<cd:constant\b([^>]*)\/>/g;
      let cm;
      while ((cm = cRe.exec(keywordBlock)) !== null) {
        const cAttrs = parseAttributes(cm[1]);
        const value = (cAttrs.value || cAttrs.type || '').trim();
        if (!value || value.startsWith('cd:')) {
          continue;
        }
        entry.keywords.add(value);
        if ((cAttrs.default || '').trim().toLowerCase() === 'yes') {
          if (!entry.parameterValueDefaults.has(name)) {
            entry.parameterValueDefaults.set(name, new Set());
          }
          entry.parameterValueDefaults.get(name).add(value);
        }
      }

      for (const refName of getReferenceNames(keywordBlock)) {
        entry.inherits.add(refName);
      }
    }

    const assignmentBlocks = block.match(/<cd:assignments\b[^>]*>[\s\S]*?<\/cd:assignments>/g) || [];
    for (const assignmentBlock of assignmentBlocks) {
      const withoutParams = assignmentBlock.replace(/<cd:parameter\b[^>]*>[\s\S]*?<\/cd:parameter>/g, '');
      for (const refName of getReferenceNames(withoutParams)) {
        entry.inherits.add(refName);
      }
    }

    const pRe = /<cd:parameter\b([^>]*)>/g;
    let pm;
    while ((pm = pRe.exec(block)) !== null) {
      const pAttrs = parseAttributes(pm[1]);
      const pName = (pAttrs.name || '').trim();
      if (!pName || pName === 'cd:key') {
        continue;
      }

      entry.parameters.add(pName);
      if (!entry.parameterValues.has(pName)) {
        entry.parameterValues.set(pName, new Set());
      }
      if (!entry.parameterTypes.has(pName)) {
        entry.parameterTypes.set(pName, new Set());
      }
      if (!entry.parameterValueDefaults.has(pName)) {
        entry.parameterValueDefaults.set(pName, new Set());
      }

      const bodyStart = pm.index + pm[0].length;
      const bodyEnd = block.indexOf('</cd:parameter>', bodyStart);
      if (bodyEnd <= bodyStart) {
        continue;
      }

      const parameterBody = block.slice(bodyStart, bodyEnd);
      const pcRe = /<cd:constant\b([^>]*)\/>/g;
      let pcm;
      while ((pcm = pcRe.exec(parameterBody)) !== null) {
        const cAttrs = parseAttributes(pcm[1]);
        const value = (cAttrs.value || cAttrs.type || '').trim();
        const explicitType = (cAttrs.type || '').trim();
        if (explicitType) {
          entry.parameterTypes.get(pName).add(explicitType);
        }
        if (!value || value.startsWith('cd:')) {
          continue;
        }
        entry.parameterValues.get(pName).add(value);
        if ((cAttrs.default || '').trim().toLowerCase() === 'yes') {
          entry.parameterValueDefaults.get(pName).add(value);
        }
      }

      if (!entry.parameterSources.has(pName)) {
        entry.parameterSources.set(pName, new Set());
      }
      for (const refName of getReferenceNames(parameterBody)) {
        entry.parameterSources.get(pName).add(refName);
      }
    }
  }

  const resolved = new Map();

  function mergeResolvedInto(target, source) {
    for (const keyword of source.keywords) {
      target.keywords.add(keyword);
    }

    for (const parameter of source.parameters) {
      target.parameters.add(parameter);
    }

    for (const [parameter, values] of source.parameterValues.entries()) {
      if (!target.parameterValues.has(parameter)) {
        target.parameterValues.set(parameter, new Set());
      }
      const targetValues = target.parameterValues.get(parameter);
      for (const value of values) {
        targetValues.add(value);
      }
    }

    for (const [parameter, types] of source.parameterTypes.entries()) {
      if (!target.parameterTypes.has(parameter)) {
        target.parameterTypes.set(parameter, new Set());
      }
      const targetTypes = target.parameterTypes.get(parameter);
      for (const value of types) {
        targetTypes.add(value);
      }
    }

    for (const [parameter, defaults] of source.parameterValueDefaults.entries()) {
      if (!target.parameterValueDefaults.has(parameter)) {
        target.parameterValueDefaults.set(parameter, new Set());
      }
      const targetDefaults = target.parameterValueDefaults.get(parameter);
      for (const value of defaults) {
        targetDefaults.add(value);
      }
    }
  }

  function resolveEntry(name, stack = new Set()) {
    if (resolved.has(name)) {
      return resolved.get(name);
    }

    if (stack.has(name) || !commandMap.has(name)) {
      return { keywords: new Set(), parameters: new Set(), parameterValues: new Map(), parameterTypes: new Map(), parameterValueDefaults: new Map() };
    }

    stack.add(name);
    const raw = commandMap.get(name);
    const out = {
      keywords: new Set(raw.keywords),
      parameters: new Set(raw.parameters),
      parameterValues: new Map(),
      parameterTypes: new Map(),
      parameterValueDefaults: new Map()
    };

    for (const [parameter, values] of raw.parameterValues.entries()) {
      out.parameterValues.set(parameter, new Set(values));
    }

    for (const [parameter, types] of raw.parameterTypes.entries()) {
      out.parameterTypes.set(parameter, new Set(types));
    }

    for (const [parameter, defaults] of raw.parameterValueDefaults.entries()) {
      out.parameterValueDefaults.set(parameter, new Set(defaults));
    }

    for (const inheritedName of raw.inherits) {
      mergeResolvedInto(out, resolveEntry(inheritedName, stack));
    }

    for (const [parameter, sources] of raw.parameterSources.entries()) {
      if (!out.parameterValues.has(parameter)) {
        out.parameterValues.set(parameter, new Set());
      }
      const valueSet = out.parameterValues.get(parameter);
      for (const sourceName of sources) {
        const sourceEntry = resolveEntry(sourceName, stack);
        for (const keyword of sourceEntry.keywords) {
          valueSet.add(keyword);
        }
      }
    }

    for (const [parameter, defaults] of raw.parameterValueDefaults.entries()) {
      if (!out.parameterValueDefaults.has(parameter)) {
        out.parameterValueDefaults.set(parameter, new Set());
      }
      const valueSet = out.parameterValueDefaults.get(parameter);
      for (const value of defaults) {
        valueSet.add(value);
      }
    }

    stack.delete(name);
    resolved.set(name, out);
    return out;
  }

  for (const key of commandMap.keys()) {
    commandMap.set(key, resolveEntry(key));
  }

  const commandCompletions = [];
  for (const [label, candidates] of completionCandidates.entries()) {
    if (!candidates || candidates.length === 0) {
      continue;
    }
    const variants = new Map();
    for (const candidate of candidates) {
      const argumentVariants = candidate.meta && candidate.meta.argumentSpecs
        ? getArgumentSpecVariants(candidate.meta.argumentSpecs)
        : [null];
      for (const argumentSpecs of argumentVariants) {
        const meta = argumentSpecs
          ? { ...candidate.meta, argumentSpecs }
          : candidate.meta;
        const commandName = meta.commandName || label.slice(1);
        const insertText = argumentSpecs
          ? (meta.type === 'environment'
            ? makeEnvironmentInsertText(commandName, argumentSpecs)
            : makeCommandInsertText(commandName, argumentSpecs))
          : candidate.insertText;
        const expanded = { ...candidate, insertText, meta };
        const key = getInsertTextValue(expanded);
        const current = variants.get(key);
        if (!current || scoreCompletionCandidate(label, expanded) > scoreCompletionCandidate(label, current)) {
          variants.set(key, expanded);
        }
      }
    }
    let variantIndex = 0;
    for (const variant of variants.values()) {
      commandCompletions.push({
        label,
        kind: variant.kind,
        insertText: variant.insertText,
        sortWeight: variant.sortWeight,
        argumentSpecs: variant.meta.argumentSpecs || [],
        variantIndex: variantIndex++
      });
    }
  }

  const commandArgumentSpecs = new Map();
  const commandArgumentSpecVariants = new Map();
  for (const [commandName, candidates] of commandArgumentSpecCandidates.entries()) {
    if (!candidates || candidates.length === 0) {
      continue;
    }

    const variants = [...new Map(candidates
      .flatMap(candidate => getArgumentSpecVariants(candidate.argumentSpecs || []))
      .map(argumentSpecs => [JSON.stringify(argumentSpecs), argumentSpecs])).values()];
    commandArgumentSpecVariants.set(commandName, variants);

    let best = candidates[0];
    for (let i = 1; i < candidates.length; i++) {
      const current = candidates[i];
      const currentIsString = current.variant === 'string';
      const bestIsString = best.variant === 'string';
      if (bestIsString && !currentIsString) {
        best = current;
        continue;
      }
      if (!best.variant && current.variant) {
        continue;
      }
      if (best.variant && !current.variant) {
        best = current;
        continue;
      }
      if (current.argumentSpecs.length > best.argumentSpecs.length) {
        best = current;
      }
    }

    commandArgumentSpecs.set(commandName, best.argumentSpecs);
  }

  return {
    commandMap,
    commandCompletions,
    commandArgumentSpecs,
    commandArgumentSpecVariants,
    structureEnvironments,
    commandMetadata
  };
}

  return { parseCommandMap };
};
