const { stripTeXComment } = require('./core/tex-text');

/** Returns structure ranges with source offsets. */
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
    const line = stripTeXComment(text.slice(lineStart, end));
    commandRe.lastIndex = 0;
    let match;
    while ((match = commandRe.exec(line)) !== null) {
      const command = `${match[1] || ''}${match[2]}`;
      const bareEnvironmentCommand = !match[1] && (match[2] === 'start' || match[2] === 'stop');
      const environment = bareEnvironmentCommand ? 'start' : `start${match[2]}`;
      const isStart = match[1] === 'start' || (bareEnvironmentCommand && match[2] === 'start');
      const isStop = match[1] === 'stop' || (bareEnvironmentCommand && match[2] === 'stop');
      if (isStart && isEnvironment(environment)) {
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
      if (!match[1] && !bareEnvironmentCommand) {
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
      if (!isStop || !isEnvironment(environment)) {
        continue;
      }
      const startIndex = stack.map(item => item.environment).lastIndexOf(environment);
      if (startIndex < 0) {
        continue;
      }
      const start = stack.splice(startIndex, 1)[0];
      const stopStart = lineStart + match.index;
      const stopEnd = stopStart + match[0].length;
      matches.push({
        ...start,
        kind: 'paired',
        stopStart,
        stopEnd,
        end: stopEnd,
        endLine: document.positionAt(stopStart).line
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
