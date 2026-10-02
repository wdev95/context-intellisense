/** Creates the ConTeXt source-data loader and command metadata services. */
module.exports = function createDataProviders(deps) {
  const { fs, path, vscode, loadInterfaceXml, expandInterfaceDefinitions, createDataCache, resolveConfiguredPath, resolveXmlPathFromTexRoot, resolveContextExecutableFromTexRoot, parseCommandMap, getCommandCompletionKind, stripTeXComment } = deps;
  // The interface XML omits NR's optional reference and suffix arguments from math-ali.mkxl.
  const supplementalInterfaceXml = `
<cd:command name="NR" referenceSourceArgument="0">
  <cd:arguments>
    <cd:keywords optional="yes">
      <cd:constant type="+"/>
      <cd:constant type="-"/>
      <cd:constant type="cd:reference"/>
    </cd:keywords>
    <cd:keywords optional="yes">
      <cd:constant type="cd:text"/>
    </cd:keywords>
  </cd:arguments>
</cd:command>`;
  let cache = deps.getCache();
  let reportDataWarning = (filePath, message) => vscode.window.setStatusBarMessage(message, 5000);

  function setDataWarningReporter(reporter) {
    if (typeof reporter === 'function') {
      reportDataWarning = reporter;
    }
  }
function cleanFontToken(token) {
  return token
    .trim()
    .replace(/^\\s!/, '')
    .replace(/^\\/, '')
    .trim();
}

function parseFontSourceFile(filePath) {
  const completions = [];
  if (!fs.existsSync(filePath)) {
    return completions;
  }

  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split(/\r?\n/);

  const styleNames = new Set();
  const alternativeNames = new Set();

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith('%')) {
      continue;
    }

    let match = line.match(/\\definefontstyle\s*\[([^\]]+)\]/);
    if (match) {
      for (const token of match[1].split(',')) {
        const name = cleanFontToken(token);
        if (name) {
          styleNames.add(name);
        }
      }
      continue;
    }

    match = line.match(/\\definefontalternative\s*\[([^\]]+)\]/);
    if (match) {
      for (const token of match[1].split(',')) {
        const name = cleanFontToken(token);
        if (name) {
          alternativeNames.add(name);
        }
      }
    }
  }

  const all = new Set();
  const sizeSuffixes = ['x', 'xx', 'a', 'b', 'c', 'd'];
  const expandable = new Set(['tf', 'bf', 'it', 'sl', 'bi', 'bs', 'sc']);

  for (const style of styleNames) {
    all.add(style);
  }

  for (const alt of alternativeNames) {
    all.add(alt);
    if (expandable.has(alt)) {
      for (const suffix of sizeSuffixes) {
        all.add(`${alt}${suffix}`);
      }
    }
  }

  all.add('tx');
  all.add('txx');

  for (const command of all) {
    completions.push({
      label: `\\${command}`,
      kind: getCommandCompletionKind({ category: 'fonts' }),
      insertText: command,
      sourcePriority: 1,
      sortWeight: '90'
    });
  }

  return completions;
}

function loadFontCompletions(xmlPath) {
  const texContextRoot = path.dirname(path.dirname(path.dirname(xmlPath)));
  const candidates = [
    path.join(texContextRoot, 'base', 'mkiv', 'font-ini.mkvi'),
    path.join(texContextRoot, 'base', 'mkiv', 'font-pre.mkiv'),
    path.join(texContextRoot, 'base', 'mkxl', 'font-ini.mklx'),
    path.join(texContextRoot, 'base', 'mkxl', 'font-pre.mkxl')
  ];

  const seen = new Set();
  const completions = [];
  for (const candidate of candidates) {
    for (const item of parseFontSourceFile(candidate)) {
      if (seen.has(item.label)) {
        continue;
      }
      seen.add(item.label);
      completions.push(item);
    }
  }

  return completions;
}

function getCommandEntry(commandName) {
  if (cache.commandMap.has(commandName)) {
    return cache.commandMap.get(commandName);
  }

  if (commandName.startsWith('start') && commandName.length > 5) {
    const baseName = commandName.slice(5);
    if (cache.commandMap.has(baseName)) {
      return cache.commandMap.get(baseName);
    }

    const setupName = `setup${baseName}`;
    if (cache.commandMap.has(setupName)) {
      return cache.commandMap.get(setupName);
    }
  }

  return null;
}

/** Loads math primitives from the active ConTeXt installation. */
function loadMathPrimitiveMetadata(xmlPath) {
  let directory = path.dirname(xmlPath);
  for (let depth = 0; depth < 8; depth += 1) {
    const mathPath = path.join(directory, 'tex', 'generic', 'context', 'luatex', 'luatex-math.tex');
    if (fs.existsSync(mathPath)) {
      const metadata = new Map();
      const source = fs.readFileSync(mathPath, 'utf8');
      const definitionRe = /\\(?:protected\s*\\)?(?:gdef|edef|xdef|def|chardef|mathchardef|Umathchardef)\s*\\([A-Za-z]+)/g;
      for (const match of source.matchAll(definitionRe)) {
        metadata.set(match[1], {
          category: 'mathematics',
          file: 'luatex-math.tex',
          level: 'primitive',
          type: ''
        });
      }
      return metadata;
    }
    directory = path.dirname(directory);
  }
  return new Map();
}

/** Returns source files that can contain active ConTeXt definitions. */
function getContextSourceFiles(xmlPath) {
  const texContextRoot = path.dirname(path.dirname(path.dirname(xmlPath)));
  const roots = [
    path.join(texContextRoot, 'base'),
    path.join(texContextRoot, 'modules'),
    path.join(path.dirname(texContextRoot), 'generic', 'context')
  ];
  const extensions = new Set(['.mkiv', '.mkvi', '.mkxl', '.mklx', '.tex']);
  const files = [];

  function visit(directory) {
    if (!fs.existsSync(directory)) {
      return;
    }
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(entryPath);
      } else if (extensions.has(path.extname(entry.name).toLowerCase())) {
        files.push(entryPath);
      }
    }
  }

  roots.forEach(visit);
  return files;
}

/** Loads directly declared ConTeXt registers from the active source tree. */
function loadRegisterMetadata(xmlPath) {
  const registerTypes = new Map([
    ['newdimension', 'dimension'],
    ['newdimen', 'dimension'],
    ['newinteger', 'integer'],
    ['newcount', 'integer'],
    ['newskip', 'skip'],
    ['newtoks', 'token'],
    ['newbox', 'box']
  ]);
  const metadata = new Map();
  const declarationRe = /\\(newdimension|newdimen|newinteger|newcount|newskip|newtoks|newbox)\s*\\([A-Za-z@:_!?]+)/g;

  for (const filePath of getContextSourceFiles(xmlPath)) {
    const source = fs.readFileSync(filePath, 'utf8')
      .split(/\r?\n/)
      .map(stripTeXComment)
      .join('\n');
    for (const match of source.matchAll(declarationRe)) {
      metadata.set(match[2], {
        category: 'register',
        file: path.basename(filePath),
        level: 'runtime',
        type: registerTypes.get(match[1]),
        kind: 'register'
      });
    }
  }

  return metadata;
}

/** Creates completion items for registers declared by the active ConTeXt source. */
function createRegisterCompletions(registerMetadata, existingLabels = new Set()) {
  return Array.from(registerMetadata, ([name, metadata]) => {
    const label = `\\${name}`;
    if (existingLabels.has(label)) {
      return null;
    }
    return {
      label,
      kind: getCommandCompletionKind(metadata),
      insertText: name,
      sourcePriority: 1,
      sortWeight: '90',
      argumentSpecs: []
    };
  }).filter(Boolean);
}

/** Merges supplemental completions without duplicating XML-defined commands. */
function mergeCompletionSources(xmlCompletions, ...supplementalSources) {
  const seen = new Set(xmlCompletions.map(completion => completion.label));
  const merged = [];
  for (const source of supplementalSources) {
    for (const completion of source) {
      if (seen.has(completion.label)) {
        continue;
      }
      seen.add(completion.label);
      merged.push(completion);
    }
  }
  return merged;
}

/** Adds semantic metadata for font alternatives discovered in ConTeXt sources. */
function addFontMetadata(fontCompletions, commandMetadata) {
  for (const completion of fontCompletions) {
    const name = String(completion.label || '').replace(/^\\/, '');
    if (name) {
      commandMetadata.set(name, {
        category: 'fonts',
        file: 'font-ini.mklx',
        level: 'style',
        type: ''
      });
    }
  }
}

  /** Extracts every contiguous workspace %D block containing command XML. */
  function extractInterfaceDocstrings(content) {
    const lines = String(content || '').split(/\r?\n/);
    const fragments = [];
    let documentation = [];

    const appendDocumentation = () => {
      const fragment = documentation.join('\n');
      if (/<cd:command\b[\s>]/.test(fragment)) {
        fragments.push(fragment);
      }
      documentation = [];
    };

    for (const line of lines) {
      const comment = line.match(/^\s*%D(?:\s?(.*))?$/);
      if (comment) {
        documentation.push(comment[1] || '');
        continue;
      }
      if (documentation.length > 0) {
        appendDocumentation();
      }
    }
    appendDocumentation();

    return fragments.join('\n');
  }

  /** Reads consecutive balanced square-bracket arguments after a macro. */
  function readBracketArguments(text, start, count) {
    const values = [];
    let cursor = start;
    while (values.length < count) {
      while (/\s/.test(text[cursor] || '') && cursor < text.length) cursor += 1;
      if (text[cursor] !== '[') break;
      const open = cursor++;
      let depth = 1;
      while (cursor < text.length && depth > 0) {
        if (text[cursor] === '[' && text[cursor - 1] !== '\\') depth += 1;
        if (text[cursor] === ']' && text[cursor - 1] !== '\\') depth -= 1;
        cursor += 1;
      }
      if (depth !== 0) break;
      values.push(text.slice(open + 1, cursor - 1));
    }
    return values;
  }

  /** Parses simple key/value assignments from a ConTeXt option list. */
  function parseOptionAssignments(text) {
    const assignments = new Map();
    const assignmentRe = /(?:^|,)\s*([A-Za-z][A-Za-z0-9_-]*)\s*=\s*([^,\]]*)/g;
    let match;
    while ((match = assignmentRe.exec(text)) !== null) {
      assignments.set(match[1].toLowerCase(), match[2].trim());
    }
    return assignments;
  }

  /** Escapes a generated value for the project interface XML fragment. */
  function escapeXml(value) {
    return String(value)
      .replace(/&/g, '&amp;')
      .replace(/"/g, '&quot;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  /** Collects keys for workspace-defined named collections. */
  function collectWorkspaceCollections(sources) {
    const collections = new Map();
    const codeSources = sources.map(source => String(source || '')
      .split(/\r?\n/)
      .map(stripTeXComment)
      .join('\n'));
    for (const source of codeSources) {
      const defineRe = /\\define[A-Za-z]*\s*\[([^\]]*)\]/g;
      let definition;
      while ((definition = defineRe.exec(source)) !== null) {
        const options = parseOptionAssignments(definition[1]);
        const name = options.get('name');
        if (!name || !/^[A-Za-z]+$/.test(name)) {
          continue;
        }
        if (!collections.has(name)) {
          collections.set(name, { keys: new Set(), unit: options.get('unit') === 'yes' });
        }
      }
    }

    for (const source of codeSources) {
      const addRe = /\\add[A-Za-z]+\b/g;
      let addition;
      while ((addition = addRe.exec(source)) !== null) {
        const [collectionName = '', key = ''] = readBracketArguments(source, addition.index + addition[0].length, 2);
        const collection = collections.get(collectionName.trim());
        if (collection && key.trim() && !/[<>"&#{}]/.test(key)) {
          collection.keys.add(key.trim());
        }
      }
    }
    return collections;
  }

  /** Expands custom cd:<collection> types into the collection's workspace keys. */
  function expandWorkspaceCollectionTypes(xml, sources) {
    const collections = collectWorkspaceCollections(sources);
    return String(xml || '').replace(/<cd:constant\b([^>]*)\/>/g, (tag, attributes) => {
      const type = attributes.match(/\btype="cd:([A-Za-z][A-Za-z0-9_-]*)"/);
      const collection = type && collections.get(type[1]);
      if (!collection || collection.keys.size === 0) return tag;
      return [...collection.keys]
        .sort((left, right) => left.localeCompare(right))
        .map(key => `<cd:constant value="${escapeXml(key)}"/>`)
        .join('\n');
    });
  }

  /** Reads project TeX sources and returns their local interface fragments. */
  async function loadWorkspaceInterfaceDocstrings() {
    if (!vscode.workspace.workspaceFolders || vscode.workspace.workspaceFolders.length === 0) {
      return '';
    }

    const files = await vscode.workspace.findFiles(
      '**/*.{tex,mkiv,mkvi,mkxl,mklx,mkxi,mkiv}',
      '**/{.git,node_modules,build,out}/**'
    );
    const sources = await Promise.all(files.map(async uri => {
      try {
        return await fs.promises.readFile(uri.fsPath, 'utf8');
      } catch {
        return '';
      }
    }));
    const docstrings = sources.map(extractInterfaceDocstrings).filter(Boolean).join('\n');
    return expandWorkspaceCollectionTypes(docstrings, sources);
  }

  async function loadData(configuredTexRootPath) {
  const texRootPath = resolveConfiguredPath(configuredTexRootPath);
  const xmlPath = resolveXmlPathFromTexRoot(texRootPath);
  if (!xmlPath) {
    cache = createDataCache();
    return;
  }

  if (cache.xmlPath === xmlPath && cache.commandCompletions.length > 0) {
    return;
  }

  let xmlText = '';
  try {
    xmlText = expandInterfaceDefinitions(loadInterfaceXml(xmlPath));
  } catch (err) {
      reportDataWarning(xmlPath, `Could not read i-context.xml. Check contextIntellisense.texRootPath.`);
      cache = createDataCache(xmlPath);
      return;
  }

  let workspaceXml = '';
  try {
    workspaceXml = await loadWorkspaceInterfaceDocstrings();
  } catch (error) {
    reportDataWarning(xmlPath, `Could not read local ConTeXt interface documentation: ${error.message || error}`);
  }

  const parsed = parseCommandMap(expandInterfaceDefinitions(`${xmlText}\n${workspaceXml}\n${supplementalInterfaceXml}`));
  const fontCompletions = loadFontCompletions(xmlPath);
  const commandMetadata = parsed.commandMetadata;
  addFontMetadata(fontCompletions, commandMetadata);
  const mathPrimitiveCompletions = [];
  for (const [name, metadata] of loadMathPrimitiveMetadata(xmlPath)) {
    if (!commandMetadata.has(name)) {
      commandMetadata.set(name, metadata);
    }
    const hasArgumentlessForm = parsed.commandCompletions.some(completion =>
      completion.label === `\\${name}` && completion.argumentSpecs.length === 0
    );
    if (!hasArgumentlessForm) {
      mathPrimitiveCompletions.push({
        label: `\\${name}`,
        kind: getCommandCompletionKind(metadata),
        insertText: `\\${name}`,
        sourcePriority: 1,
        sortWeight: '90',
        argumentSpecs: []
      });
    }
  }
  const registerMetadata = loadRegisterMetadata(xmlPath);
  for (const [name, metadata] of registerMetadata) {
    if (!commandMetadata.has(name)) {
      commandMetadata.set(name, metadata);
    }
  }
  const registerCompletions = createRegisterCompletions(registerMetadata);
  const supplementalCompletions = mergeCompletionSources(
    parsed.commandCompletions,
    fontCompletions,
    mathPrimitiveCompletions,
    registerCompletions
  );
  cache = {
    xmlPath,
    commandMap: parsed.commandMap,
    commandCompletions: parsed.commandCompletions.concat(supplementalCompletions),
    commandArgumentSpecs: parsed.commandArgumentSpecs,
    commandArgumentSpecVariants: parsed.commandArgumentSpecVariants,
    structureEnvironments: parsed.structureEnvironments,
    commandMetadata,
    parameterValueDefaults: new Map(Array.from(parsed.commandMap.entries()).map(([name, entry]) => [name, entry.parameterValueDefaults || new Map()]))
  };
}

/** Returns whether a delimiter at a source position is escaped. */
  return { cleanFontToken, parseFontSourceFile, loadFontCompletions, getCommandEntry, loadMathPrimitiveMetadata, getContextSourceFiles, loadRegisterMetadata, createRegisterCompletions, addFontMetadata, loadData, setDataWarningReporter, getCache: () => cache };
};
