const fs = require('fs');
const path = require('path');

/** Loads an interface tree while preserving commented definitions. */
function loadInterfaceXml(filePath, visited = new Set()) {
  const absolutePath = path.resolve(filePath);
  if (visited.has(absolutePath) || !fs.existsSync(absolutePath)) {
    return '';
  }

  visited.add(absolutePath);
  const text = fs.readFileSync(absolutePath, 'utf8');
  const references = [...text.matchAll(/<cd:interfacefile\b[^>]*\bfilename="([^"]+)"[^>]*\/?>/g)];
  return text + references
    .map(([, filename]) => loadInterfaceXml(path.join(path.dirname(absolutePath), filename), visited))
    .join('\n');
}

/** Extracts mathematical command names from interface XML text. */
function readMathXmlNames(text) {
  const names = new Set();
  const blocks = text.match(/<cd:command\b[^>]*\/>|<cd:command\b[^>]*>[\s\S]*?<\/cd:command>/g) || [];
  for (const block of blocks) {
    const opening = block.match(/^<cd:command\b[^>]*\/?\s*>/);
    if (!opening) {
      continue;
    }
    const category = /\bcategory="([^"]+)"/.exec(opening[0]);
    if (!category || category[1].toLowerCase() !== 'mathematics') {
      continue;
    }

    const command = /\bname="([A-Za-z@:_!?]+)"/.exec(opening[0]);
    if (command) {
      names.add(command[1]);
    }
    for (const instance of block.matchAll(/<cd:constant\s+value="([A-Za-z@:_!?]+)"/g)) {
      names.add(instance[1]);
    }

    if (/\btype="environment"/.test(opening[0])) {
      for (const name of [...names]) {
        if (block.includes(`value="${name}"`)) {
          names.add(`start${name}`);
          names.add(`stop${name}`);
        }
      }
      if (command) {
        names.add(`start${command[1]}`);
        names.add(`stop${command[1]}`);
      }
    }
  }
  return names;
}

/** Extracts mathematical character command names from char-def.lua. */
function readMathCharacterNames(filePath) {
  const names = new Set();
  if (!fs.existsSync(filePath)) {
    return names;
  }

  const text = fs.readFileSync(filePath, 'utf8');
  const pattern = /class\s*=\s*"(?:binary|relation|ordinary|punctuation|opening|closing|inner|accent|radical|fraction|large|over|under|operator)"[\s\S]{0,300}?name\s*=\s*"([A-Za-z@:_!?]+)"/g;
  for (const match of text.matchAll(pattern)) {
    names.add(match[1]);
  }
  return names;
}

/** Replaces a byte range while preserving the grammar file encoding. */
function replaceRange(source, start, length, replacement) {
  return Buffer.concat([source.subarray(0, start), replacement, source.subarray(start + length)]);
}

/** Finds a byte sequence in a buffer. */
function findBytes(source, pattern, start = 0) {
  for (let index = start; index <= source.length - pattern.length; index++) {
    if (pattern.every((byte, offset) => source[index + offset] === byte)) {
      return index;
    }
  }
  return -1;
}

/** Generates the mathematical TextMate rule from the installed ConTeXt sources. */
function main() {
  const texRoot = process.env.CONTEXT_TEX_ROOT;
  if (!texRoot) {
    throw new Error('Set CONTEXT_TEX_ROOT before generating the grammar.');
  }

  const interfaceDirectory = path.join(texRoot, 'tex', 'context', 'interface', 'mkiv');
  const baseDirectory = path.join(texRoot, 'tex', 'context', 'base', 'mkiv');
  const interfaceXml = loadInterfaceXml(path.join(interfaceDirectory, 'i-context.xml'));
  const names = new Set([
    ...readMathXmlNames(interfaceXml),
    ...readMathXmlNames(fs.readFileSync(path.join(interfaceDirectory, 'context-en.xml'), 'utf8')),
    ...readMathCharacterNames(path.join(baseDirectory, 'char-def.lua'))
  ]);
  const sortedNames = [...names].filter((name) => /^[A-Za-z@:_!?]+$/.test(name)).sort((a, b) => b.length - a.length);
  const grammarPath = path.resolve(__dirname, '..', 'syntaxes', 'context-syntax-tex.json');
  let grammar = fs.readFileSync(grammarPath);
  const mathStart = Buffer.from('  "math_command" : {');
  const mathEnd = Buffer.from('\r\n  },\r\n  "command" : {');
  const mathRuleStart = findBytes(grammar, mathStart);
  const mathRuleEnd = findBytes(grammar, mathEnd, mathRuleStart);
  if (mathRuleStart < 0 || mathRuleEnd < 0) {
    throw new Error('Could not locate the mathematical command grammar rule.');
  }
  const patterns = [];
  for (let index = 0; index < sortedNames.length; index += 100) {
    const namesChunk = sortedNames.slice(index, index + 100).join('|');
    patterns.push(
      `    {\r\n     "match" : "\\\\\\\\(${namesChunk})(?=[^a-zA-Z])",\r\n     "name" : "storage.type.math.context.tex"\r\n    }`
    );
  }
  const generatedMathRule = Buffer.from(
    `  "math_command" : {\r\n   "patterns" : [\r\n${patterns.join(',\r\n')}\r\n   ]\r\n`
  );
  grammar = replaceRange(grammar, mathRuleStart, mathRuleEnd - mathRuleStart, generatedMathRule);
  const commandStart = Buffer.from('  "command" : {\r\n   "match" : "');
  const commandEnd = Buffer.from('",\r\n   "name" : "entity.name.function.commands.context.tex"');
  const matchStart = findBytes(grammar, commandStart);
  const matchEnd = findBytes(grammar, commandEnd, matchStart);
  if (matchStart < 0 || matchEnd < 0) {
    throw new Error('Could not locate the generic command grammar rule.');
  }

  const genericCommand = Buffer.from('  "command" : {\r\n   "match" : "\\\\\\\\[A-Za-z@:_!?]+(?=[^a-zA-Z])');
  grammar = replaceRange(grammar, matchStart, matchEnd - matchStart, genericCommand);
  fs.writeFileSync(grammarPath, grammar);
  console.log(`Generated ${sortedNames.length} mathematical command names.`);
}

main();
