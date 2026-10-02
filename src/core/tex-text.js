/** Removes an unescaped TeX comment from one source line. */
function stripTeXComment(line) {
  for (let index = 0; index < line.length; index += 1) {
    if (line[index] === '%' && (index === 0 || line[index - 1] !== '\\')) {
      return line.slice(0, index);
    }
  }
  return line;
}

module.exports = { stripTeXComment };
