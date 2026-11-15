/** Returns whether a character is escaped by an odd run of backslashes. */
function isEscaped(text, index) {
  let slashes = 0;
  while (index > 0 && text[--index] === '\\') slashes += 1;
  return slashes % 2 === 1;
}

/** Finds the first unescaped TeX comment marker. */
function findCommentStart(line) {
  for (let index = 0; index < line.length; index += 1) {
    if (line[index] === '%' && !isEscaped(line, index)) return index;
  }
  return -1;
}

/** Removes an unescaped TeX comment from one source line. */
function stripTeXComment(line) {
  const start = findCommentStart(line);
  return start < 0 ? line : line.slice(0, start);
}

/** Masks comments without changing source offsets or line endings. */
function maskTeXComments(text) {
  const characters = text.split('');
  for (let index = 0; index < characters.length; index += 1) {
    if (characters[index] !== '%' || isEscaped(text, index)) continue;
    while (index < characters.length && characters[index] !== '\n' && characters[index] !== '\r') {
      characters[index++] = ' ';
    }
  }
  return characters.join('');
}

module.exports = { findCommentStart, isEscaped, maskTeXComments, stripTeXComment };
