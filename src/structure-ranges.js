/** Returns XML-defined structure ranges with source offsets. */
module.exports = function getStructureEnvironmentMatches(document, isEnvironment, getStandaloneRank) {
  const text = document.getText();
  const stack = [];
  const matches = [];
  const starts = [];
  const commandRe = /\\(start|stop)?([A-Za-z]+)/g;
  let lineStart = 0;

  while (lineStart <= text.length) {
    const lineEnd = text.indexOf('\n', lineStart);
    const end = lineEnd < 0 ? text.length : lineEnd;
    const line = maskTeXComment(text.slice(lineStart, end));
    commandRe.lastIndex = 0;
    let match;
    while ((match = commandRe.exec(line)) !== null) {
      const command = `${match[1] || ''}${match[2]}`;
      const environment = `start${match[2]}`;
      if (match[1] === 'start' && isEnvironment(command)) {
        const start = {
          command,
          environment,
          start: lineStart + match.index,
          startEnd: lineStart + match.index + match[0].length,
          startLine: document.positionAt(lineStart + match.index).line,
          rank: getStandaloneRank(command)
        };
        stack.push(start);
        starts.push({ ...start, kind: 'paired' });
        continue;
      }
      if (!match[1]) {
        const rank = getStandaloneRank(command);
        if (Number.isInteger(rank)) {
          starts.push({
            kind: 'standalone',
            command,
            start: lineStart + match.index,
            startEnd: lineStart + match.index + match[0].length,
            startLine: document.positionAt(lineStart + match.index).line,
            rank
          });
        }
        continue;
      }
      if (match[1] !== 'stop' || !isEnvironment(environment)) {
        continue;
      }
      const startIndex = stack.map(item => item.environment).lastIndexOf(environment);
      if (startIndex < 0) {
        continue;
      }
      const start = stack.splice(startIndex, 1)[0];
      matches.push({
        ...start,
        kind: 'paired',
        end: lineStart + match.index + match[0].length,
        endLine: document.positionAt(lineStart + match.index).line
      });
    }
    if (lineEnd < 0) {
      break;
    }
    lineStart = lineEnd + 1;
  }
  for (const start of starts.filter(item => item.kind === 'standalone')) {
    const next = starts
      .filter(item => item.start > start.start && item.rank <= start.rank)
      .sort((left, right) => left.start - right.start)[0];
    const end = next ? next.start : text.length;
    matches.push({
      ...start,
      end,
      endLine: document.positionAt(Math.max(start.start, end - 1)).line
    });
  }
  return matches.sort((left, right) => left.start - right.start);
};

/** Masks unescaped TeX comments without changing source offsets. */
function maskTeXComment(line) {
  for (let index = 0; index < line.length; index += 1) {
    if (line[index] !== '%' || (index > 0 && line[index - 1] === '\\')) {
      continue;
    }
    return `${line.slice(0, index)}${' '.repeat(line.length - index)}`;
  }
  return line;
}
