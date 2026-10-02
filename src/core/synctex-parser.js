/** Parses the first SyncTeX view result into a PDF page location. */
function parseSyncTexViewResult(output) {
  const values = {};
  for (const line of String(output || '').split(/\r?\n/)) {
    const match = /^(Page|x|y):\s*(-?\d+(?:\.\d+)?)/i.exec(line.trim());
    if (match) {
      values[match[1].toLowerCase()] = Number(match[2]);
      if (Number.isSafeInteger(values.page) && Number.isFinite(values.x) && Number.isFinite(values.y)) {
        return { pageNumber: values.page, x: values.x, y: values.y };
      }
    }
  }
  return null;
}

/** Parses a SyncTeX edit result into a source file location. */
function parseSyncTexEditResult(output) {
  const locations = [];
  let current = null;
  for (const rawLine of String(output || '').split(/\r?\n/)) {
    const line = rawLine.trim();
    const input = /^Input:\s*(?:\d+:)?(.+)$/i.exec(line);
    if (input) {
      if (current && current.line !== undefined) {
        locations.push(current);
      }
      current = { filePath: input[1].trim(), line: undefined, column: 0 };
      continue;
    }
    if (!current) {
      continue;
    }
    const sourceLine = /^Line:\s*(-?\d+)$/i.exec(line);
    if (sourceLine) {
      current.line = Math.max(Number(sourceLine[1]), 1);
      continue;
    }
    const column = /^Column:\s*(-?\d+)$/i.exec(line);
    if (column) {
      current.column = Math.max(Number(column[1]), 0);
    }
  }
  if (current && current.line !== undefined) {
    locations.push(current);
  }
  return locations.length > 0 ? locations[0] : null;
}

/** Parses the ConTeXt mtx-synctex forward result. */
function parseMtxSyncTexFindResult(output) {
  const match = /page\s*=\s*(\d+)\s+llx\s*=\s*(-?\d+(?:\.\d+)?)\s+lly\s*=\s*(-?\d+(?:\.\d+)?)/i.exec(String(output || ''));
  return match ? { pageNumber: Number(match[1]), x: Number(match[2]), y: Number(match[3]) } : null;
}

/** Parses the ConTeXt mtx-synctex inverse result. */
function parseMtxSyncTexReportResult(output) {
  const match = /^\s*["'](.+)["']\s+(-?\d+)\s+(-?\d+)\s*$/m.exec(String(output || ''));
  return match ? {
    filePath: match[1].trim(),
    line: Math.max(Number(match[2]), 1),
    column: Math.max(Number(match[3]), 0)
  } : null;
}

module.exports = {
  parseSyncTexViewResult,
  parseSyncTexEditResult,
  parseMtxSyncTexFindResult,
  parseMtxSyncTexReportResult
};
