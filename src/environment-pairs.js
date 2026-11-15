const getStructureEnvironmentMatches = require('./structure-ranges');

module.exports = function createEnvironmentPairs({ vscode }) {
  function getPairs(document) {
    return getStructureEnvironmentMatches(
      document,
      command => /^start[A-Za-z]*$/.test(command),
      () => undefined
    ).filter(match => Number.isInteger(match.stopStart));
  }

  function getPairAt(document, offset) {
    return getPairs(document).find(pair =>
      (offset >= pair.start && offset <= pair.startEnd)
      || (offset >= pair.stopStart && offset <= pair.stopEnd));
  }

  function getNameRanges(document, pair) {
    return [
      new vscode.Range(document.positionAt(pair.start + 6), document.positionAt(pair.startEnd)),
      new vscode.Range(document.positionAt(pair.stopStart + 5), document.positionAt(pair.stopEnd))
    ];
  }

  function prepareCompletionItems(document, position, items) {
    const offset = document.offsetAt(position);
    const pair = getPairAt(document, offset);
    if (!pair || offset < pair.start + 6 || offset > pair.startEnd) return items;

    const stopRange = new vscode.Range(
      document.positionAt(pair.stopStart),
      document.positionAt(pair.stopEnd)
    );
    const startNameRange = new vscode.Range(
      document.positionAt(pair.start + 1),
      document.positionAt(pair.startEnd)
    );
    for (const item of items) {
      const commandName = String(typeof item.label === 'string' ? item.label : item.label.label)
        .match(/^\\?([A-Za-z]+)/)?.[1];
      if (!commandName || !commandName.startsWith('start')) continue;

      const environmentName = commandName.slice(5);
      const insertion = item.insertText;
      if (typeof insertion?.value === 'string') {
        const commandAndArguments = insertion.value.split(/\r?\n/, 1)[0];
        item.insertText = new vscode.SnippetString(commandAndArguments);
      } else if (typeof insertion === 'string') {
        item.insertText = insertion.split(/\r?\n/, 1)[0];
      }
      item.range = startNameRange;
      item.additionalTextEdits = [new vscode.TextEdit(stopRange, `\\stop${environmentName}`)];
    }
    return items;
  }

  return {
    prepareCompletionItems,

    getHighlightRanges(document, position) {
      const pair = getPairAt(document, document.offsetAt(position));
      if (!pair) return [];
      return [
        new vscode.Range(document.positionAt(pair.start), document.positionAt(pair.startEnd)),
        new vscode.Range(document.positionAt(pair.stopStart), document.positionAt(pair.stopEnd))
      ];
    },

    provideLinkedEditingRanges(document, position) {
      const offset = document.offsetAt(position);
      const pair = getPairAt(document, offset);
      if (!pair) return undefined;
      const ranges = getNameRanges(document, pair);
      if (!ranges.some(range => offset >= document.offsetAt(range.start) && offset <= document.offsetAt(range.end))) {
        return undefined;
      }
      return new vscode.LinkedEditingRanges(ranges, /^[A-Za-z]*$/);
    }
  };
};
