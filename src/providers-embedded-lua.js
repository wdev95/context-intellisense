const SCHEME = 'context-embedded-lua';
const LUA_SERVER_ID = 'sumneko.lua';

/** Returns whether a source character is escaped by an odd backslash run. */
function isEscaped(text, index) {
  let count = 0;
  while (index > 0 && text[--index] === '\\') count++;
  return count % 2 === 1;
}

/** Reads a Lua long-bracket delimiter beginning at the given offset. */
function getLongBracket(text, offset) {
  const match = /^\[(=*)\[/.exec(text.slice(offset));
  return match ? { contentStart: offset + match[0].length, close: `]${match[1]}]` } : null;
}

/** Finds the matching Lua stop command while skipping Lua strings and comments. */
function findLuaStop(text, offset) {
  for (let index = offset; index < text.length; index++) {
    const character = text[index];
    if (character === '"' || character === "'") {
      const quote = character;
      while (++index < text.length) {
        if (text[index] === '\\') index++;
        else if (text[index] === quote) break;
      }
      continue;
    }
    if (character === '[') {
      const longBracket = getLongBracket(text, index);
      if (longBracket) {
        const close = text.indexOf(longBracket.close, longBracket.contentStart);
        if (close < 0) return text.length;
        index = close + longBracket.close.length - 1;
        continue;
      }
    }
    if (character === '-' && text[index + 1] === '-') {
      const longComment = getLongBracket(text, index + 2);
      if (longComment) {
        const close = text.indexOf(longComment.close, longComment.contentStart);
        if (close < 0) return text.length;
        index = close + longComment.close.length - 1;
      } else {
        while (index < text.length && text[index] !== '\n') index++;
      }
      continue;
    }
    if (character === '\\' && !isEscaped(text, index)
      && text.startsWith('\\stopluacode', index)
      && !/[A-Za-z]/.test(text[index + 12] || '')) {
      return index;
    }
  }
  return text.length;
}

/** Finds complete and currently-open ConTeXt Lua code regions. */
function findLuaBlocks(text) {
  const blocks = [];
  for (let index = 0; index < text.length;) {
    if (text[index] === '%' && !isEscaped(text, index)) {
      while (index < text.length && text[index] !== '\n') index++;
      continue;
    }
    if (text[index] === '\\' && !isEscaped(text, index)
      && text.startsWith('\\startluacode', index)
      && !/[A-Za-z]/.test(text[index + 13] || '')) {
      const start = index + 13;
      const stop = findLuaStop(text, start);
      blocks.push({ start, end: stop });
      if (stop === text.length) break;
      index = stop + 12;
      continue;
    }
    index++;
  }
  return blocks;
}

/** Masks TeX outside Lua blocks while preserving every source position. */
function createLuaDocument(text, blocks = findLuaBlocks(text)) {
  const characters = text.split('');
  const keep = new Uint8Array(text.length);
  for (const { start, end } of blocks) {
    keep.fill(1, start, end);
  }
  for (let index = 0; index < characters.length; index++) {
    if (!keep[index] && characters[index] !== '\n' && characters[index] !== '\r') {
      characters[index] = ' ';
    }
  }
  return characters.join('');
}

/** Creates request forwarding from ConTeXt Lua blocks to the installed LuaLS. */
function createEmbeddedLuaBridge(vscode) {
  const contents = new Map();
  const virtualUris = new Map();
  const sourceStates = new Map();
  const emitter = new vscode.EventEmitter();
  const provider = vscode.workspace.registerTextDocumentContentProvider(SCHEME, {
    onDidChange: emitter.event,
    provideTextDocumentContent: uri => contents.get(uri.toString()) || ''
  });
  const closeListener = vscode.workspace.onDidCloseTextDocument(document => {
    const sourceKey = document.uri.toString();
    const uri = virtualUris.get(sourceKey);
    sourceStates.delete(sourceKey);
    virtualUris.delete(sourceKey);
    if (uri) {
      contents.delete(uri.toString());
      emitter.fire(uri);
    }
  });

  /** Returns a stable Lua URI for one source document. */
  function getVirtualUri(document) {
    const key = document.uri.toString();
    if (!virtualUris.has(key)) {
      virtualUris.set(key, vscode.Uri.parse(
        `${SCHEME}://embedded/${encodeURIComponent(key)}.lua`
      ));
    }
    return virtualUris.get(key);
  }

  /** Opens and synchronizes the Lua view when the cursor is inside a block. */
  async function getLuaTarget(document, position) {
    if (!vscode.extensions.getExtension(LUA_SERVER_ID)) return null;
    const sourceKey = document.uri.toString();
    let state = sourceStates.get(sourceKey);
    if (!state || state.version !== document.version) {
      const source = document.getText();
      const blocks = findLuaBlocks(source);
      state = { version: document.version, blocks, content: blocks.length ? createLuaDocument(source, blocks) : '' };
      sourceStates.set(sourceKey, state);
    }
    const offset = document.offsetAt(position);
    if (!state.blocks.some(({ start, end }) => offset >= start
      && (offset < end || (end === source.length && offset === end)))) {
      return null;
    }

    const uri = getVirtualUri(document);
    const key = uri.toString();
    const content = state.content;
    if (contents.get(key) !== content) {
      const changed = contents.has(key);
      contents.set(key, content);
      if (changed) emitter.fire(uri);
    }
    let luaDocument = await vscode.workspace.openTextDocument(uri);
    if (luaDocument.languageId !== 'lua') {
      luaDocument = await vscode.languages.setTextDocumentLanguage(luaDocument, 'lua');
    }
    return { uri, position, sourceUri: document.uri, luaDocument };
  }

  /** Executes a VS Code language-provider request against the Lua document. */
  async function forward(command, document, position, ...args) {
    const target = await getLuaTarget(document, position);
    return target ? vscode.commands.executeCommand(command, target.uri, target.position, ...args) : undefined;
  }

  /** Forwards Lua completion requests, returning undefined outside Lua blocks. */
  async function provideCompletionItems(document, position, triggerCharacter) {
    const result = await forward('vscode.executeCompletionItemProvider', document, position, triggerCharacter);
    return result === undefined ? undefined : (result?.items || result || []);
  }

  /** Forwards Lua signature help when the cursor is inside a Lua block. */
  function provideSignatureHelp(document, position, triggerCharacter) {
    return forward('vscode.executeSignatureHelpProvider', document, position, triggerCharacter);
  }

  /** Forwards Lua hover requests when the cursor is inside a Lua block. */
  function provideHover(document, position) {
    return forward('vscode.executeHoverProvider', document, position);
  }

  return {
    provideCompletionItems,
    provideSignatureHelp,
    provideHover,
    /** Releases virtual documents and listeners when the extension deactivates. */
    dispose() {
      provider.dispose();
      closeListener.dispose();
      emitter.dispose();
      contents.clear();
      virtualUris.clear();
      sourceStates.clear();
    }
  };
}

module.exports = createEmbeddedLuaBridge;
module.exports.findLuaBlocks = findLuaBlocks;
module.exports.createLuaDocument = createLuaDocument;
