const path = require('path');

const VIEW_ID = 'contextIntellisense.settingsView';
const SECTION = 'contextIntellisense';
const FONT_SIZE_MIME = `application/vnd.code.tree.${VIEW_ID.toLowerCase()}`;
const FONT_SIZE_ORDER_KEY = 'fontSizeOrder';
const FONT_BUTTONS = {
  bold: ['Bold', 'bold'],
  emphasize: ['Emphasize', 'italic'],
  color: ['Text Color', 'symbol-color'],
  fontSize: ['Font Sizes', 'text-size'],
  highlight: ['Highlight', 'paintcan']
};

/** Registers the native settings tree and applies edits to VS Code configuration. */
module.exports = function registerSettingsTree(vscode, context) {
  const schemas = new Map();
  const groups = (context.extension.packageJSON.contributes.configuration || []).map(group => ({
    id: group.id,
    title: group.title,
    settings: Object.entries(group.properties || {}).map(([id, schema]) => {
      const setting = {
        id,
        path: id.slice(`${SECTION}.`.length),
        title: settingTitle(id),
        type: schema.type,
        scope: schema.scope || 'window',
        description: schema.description || '',
        default: schema.default,
        enum: schema.enum || [],
        enumDescriptions: schema.enumDescriptions || []
      };
      schemas.set(setting.path, setting);
      return setting;
    })
  }));
  const fontGroup = groups.find(group => group.id === `${SECTION}.fontSwitches`);
  const fontSettings = new Map(fontGroup.settings.map(setting => [setting.path, setting]));
  const changed = new vscode.EventEmitter();
  const configuration = () => vscode.workspace.getConfiguration(SECTION);
  let target = context.globalState.get('contextIntellisense.settingsTarget')
    || (vscode.workspace.workspaceFolders?.length ? 'workspace' : 'user');
  if (!vscode.workspace.workspaceFolders?.length) target = 'user';
  let tree;
  let writeQueue = Promise.resolve();

  const provider = {
    onDidChangeTreeData: changed.event,
    getChildren(element) {
      if (!element) return groups.map(group => ({ kind: 'group', group }));
      if (element.kind === 'group') return childrenForGroup(element.group);
      if (element.kind === 'fontSizes') return configurationObjectEntries(element.setting)
        .map(([name, template]) => ({ kind: 'fontSizeEntry', name, template }));
      return [];
    },
    getTreeItem(node) {
      const expandable = node.kind === 'group' || node.kind === 'fontSizes';
      const item = new vscode.TreeItem(node.label || labelForNode(node), expandable
        ? vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.None);
      item.iconPath = new vscode.ThemeIcon(node.icon || iconForNode(node));
      item.description = descriptionForNode(node);
      item.tooltip = tooltipForNode(node);
      if (node.kind === 'fontSizeEntry') {
        item.id = `fontSize:${node.name}`;
        item.contextValue = 'contextIntellisense.fontSizeEntry';
      } else if (node.kind === 'setting' || node.kind === 'fontSizes') {
        item.contextValue = node.kind === 'fontSizes'
          ? 'contextIntellisense.fontSizeCollection'
          : node.contextValue;
        if (node.checkboxPath) {
          item.checkboxState = {
            state: configuration().get(node.checkboxPath, true)
              ? vscode.TreeItemCheckboxState.Checked
              : vscode.TreeItemCheckboxState.Unchecked,
            tooltip: `Show or hide the ${node.label || node.setting.title} button`
          };
        }
      }
      return item;
    },
    dragMimeTypes: [FONT_SIZE_MIME],
    dropMimeTypes: [FONT_SIZE_MIME],
    handleDrag(source, transfer) {
      if (source.length && source.every(node => node.kind === 'fontSizeEntry')) {
        transfer.set(FONT_SIZE_MIME, new vscode.DataTransferItem(source));
      }
    },
    async handleDrop(targetNode, transfer) {
      const dragged = transfer.get(FONT_SIZE_MIME)?.value;
      if (!Array.isArray(dragged) || !dragged.length
        || (targetNode && !['fontSizeEntry', 'fontSizes'].includes(targetNode.kind))) return;
      const setting = fontSettings.get('fontSwitches.fontSizes');
      const entries = configurationObjectEntries(setting);
      const movingNames = new Set(dragged
        .filter(node => node?.kind === 'fontSizeEntry' && entries.some(([key]) => key === node.name))
        .map(node => node.name));
      if (!movingNames.size || (targetNode?.kind === 'fontSizeEntry' && movingNames.has(targetNode.name))) return;
      const moving = entries.filter(([key]) => movingNames.has(key));
      const remaining = entries.filter(([key]) => !movingNames.has(key));
      const targetIndex = targetNode?.kind === 'fontSizeEntry'
        ? remaining.findIndex(([key]) => key === targetNode.name)
        : remaining.length;
      remaining.splice(targetIndex < 0 ? remaining.length : targetIndex, 0, ...moving);
      try {
        await updateFontSizes(remaining);
      } catch (error) {
        vscode.window.showErrorMessage(`Could not reorder font sizes: ${error.message || error}`);
      }
    }
  };

  function childrenForGroup(group) {
    if (group === fontGroup) return fontSwitchChildren();
    return group.settings.map(setting => settingNode(setting));
  }

  function fontSwitchChildren() {
    const handled = new Set();
    const buttonNode = (key, settingPath) => {
      const [label, icon] = FONT_BUTTONS[key];
      const buttonSetting = fontSettings.get(`fontSwitches.buttons.${key}`);
      if (!buttonSetting) return undefined;
      handled.add(buttonSetting.path);
      if (settingPath) handled.add(settingPath);
      return {
        kind: 'setting',
        label,
        icon,
        path: settingPath || buttonSetting.path,
        editPath: settingPath,
        checkboxPath: buttonSetting.path,
        resetPaths: settingPath ? [settingPath, buttonSetting.path] : [buttonSetting.path],
        setting: settingPath ? fontSettings.get(settingPath) : buttonSetting,
        contextValue: settingPath ? 'contextIntellisense.setting' : 'contextIntellisense.toggleSetting',
        description: settingPath ? valueDescription(fontSettings.get(settingPath)) : buttonSetting.description
      };
    };

    [...fontSettings.values()]
      .filter(setting => /^fontSwitches\.fontSize(?:Xx|X|Normal|A|B|C|D)$/.test(setting.path))
      .forEach(setting => handled.add(setting.path));

    const children = [
      buttonNode('bold', 'fontSwitches.bold'),
      buttonNode('emphasize', 'fontSwitches.emphasize'),
      buttonNode('color', 'fontSwitches.color'),
      {
        kind: 'fontSizes',
        label: 'Font Sizes',
        icon: FONT_BUTTONS.fontSize[1],
        setting: fontSettings.get('fontSwitches.fontSizes'),
        checkboxPath: fontSettings.get('fontSwitches.buttons.fontSize')?.path,
        path: 'fontSwitches.fontSizes',
        resetPaths: ['fontSwitches.fontSizes', 'fontSwitches.buttons.fontSize']
      },
      buttonNode('highlight')
    ].filter(Boolean);
    for (const node of children) {
      if (node.kind === 'fontSizes') {
        handled.add(node.setting.path);
        handled.add(node.checkboxPath);
      }
    }
    return children.concat([...fontSettings.values()]
      .filter(setting => !handled.has(setting.path))
      .map(setting => settingNode(setting)));
  }

  function settingNode(setting) {
    return {
      kind: 'setting',
      setting,
      path: setting.path,
      label: setting.title,
      checkboxPath: setting.type === 'boolean' ? setting.path : undefined,
      contextValue: setting.type === 'boolean' ? 'contextIntellisense.toggleSetting' : 'contextIntellisense.setting',
      resetPaths: [setting.path]
    };
  }

  function labelForNode(node) {
    if (node.kind === 'group') return node.group.title;
    if (node.kind === 'fontSizeEntry') return node.name;
    return node.setting?.title || '';
  }

  function valueDescription(setting) {
    if (setting.type === 'object') return `${configurationObjectEntries(setting).length} entries`;
    const value = configuration().get(setting.path, setting.default);
    if (setting.type === 'boolean') return value ? 'Enabled' : 'Disabled';
    return String(value || 'Not set');
  }

  function descriptionForNode(node) {
    if (node.kind === 'group') return '';
    if (node.kind === 'fontSizeEntry') return node.template;
    if (node.kind === 'fontSizes') return valueDescription(node.setting);
    if (node.setting?.type === 'boolean') return '';
    return node.description || valueDescription(node.setting);
  }

  function iconForNode(node) {
    if (node.kind === 'group') return ({
      'Distribution': 'package',
      'Compilation & PDF': 'gear',
      'Explorer': 'folder',
      'SyncTeX': 'sync',
      'Font Switches': 'symbol-namespace'
    })[node.group.title] || 'settings-gear';
    if (node.kind === 'fontSizeEntry') return 'text-size';
    if (node.setting?.path === 'distribution.texRootPath') return 'folder';
    if (node.setting?.path === 'compilation.mainFilePath') return 'file';
    if (node.setting?.type === 'boolean') return 'symbol-boolean';
    if (node.setting?.enum.length) return 'list-selection';
    return 'symbol-string';
  }

  function tooltipForNode(node) {
    if (node.kind === 'group') return node.group.title;
    if (node.kind === 'fontSizeEntry') return `${node.name}: ${node.template}`;
    if (node.kind === 'fontSizes') return node.label;
    const setting = node.setting;
    return [setting?.id, setting?.scope === 'machine' ? 'User scope' : '', setting?.description,
      node.checkboxPath ? `Button setting: ${SECTION}.${node.checkboxPath}` : '']
      .filter(Boolean).join('\n');
  }

  function configTarget(setting) {
    return setting.scope === 'machine' || target !== 'workspace' || !vscode.workspace.workspaceFolders?.length
      ? vscode.ConfigurationTarget.Global
      : vscode.ConfigurationTarget.Workspace;
  }

  function configurationObjectEntries(setting) {
    const inspected = configuration().inspect(setting.path) || {};
    const layers = [inspected.defaultValue ?? setting.default, inspected.globalValue];
    if (target === 'workspace') layers.push(inspected.workspaceValue);
    const objects = layers.filter(value => value && typeof value === 'object' && !Array.isArray(value));
    const values = Object.assign({}, ...objects);
    const order = [];
    for (const object of objects) {
      const keys = Object.keys(object);
      if (order.every(key => Object.prototype.hasOwnProperty.call(object, key))) order.splice(0, order.length, ...keys);
      else for (const key of keys) if (!order.includes(key)) order.push(key);
    }
    const storedOrder = fontSizeOrderStorage().get(FONT_SIZE_ORDER_KEY, []);
    const keys = [...new Set([
      ...storedOrder.filter(key => Object.prototype.hasOwnProperty.call(values, key)),
      ...order
    ])];
    return keys.filter(key => Object.prototype.hasOwnProperty.call(values, key)).map(key => [key, values[key]]);
  }

  function fontSizeOrderStorage() {
    return target === 'workspace' ? context.workspaceState : context.globalState;
  }

  async function updateFontSizes(entries) {
    const setting = fontSettings.get('fontSwitches.fontSizes');
    const storage = fontSizeOrderStorage();
    const previousOrder = storage.get(FONT_SIZE_ORDER_KEY);
    await storage.update(FONT_SIZE_ORDER_KEY, entries.map(([name]) => name));
    try {
      await update(setting.path, Object.fromEntries(entries));
    } catch (error) {
      await storage.update(FONT_SIZE_ORDER_KEY, previousOrder);
      throw error;
    }
  }

  async function update(pathName, value) {
    const setting = schemas.get(pathName);
    if (!setting) return;
    await configuration().update(pathName, value, configTarget(setting));
    changed.fire(undefined);
  }

  async function editSetting(pathName) {
    const setting = schemas.get(pathName);
    if (!setting) return;
    if (setting.enum.length) {
      const current = configuration().get(pathName, setting.default);
      const selection = await vscode.window.showQuickPick(setting.enum.map((value, index) => ({
        label: value,
        description: value === current ? 'Current value' : '',
        detail: setting.enumDescriptions[index]
      })), { placeHolder: setting.title, title: 'Select ' + setting.title });
      if (selection) await update(pathName, selection.label);
      return;
    }
    if (pathName === 'distribution.texRootPath') {
      const current = String(configuration().get(pathName, '') || '');
      const selected = await vscode.window.showOpenDialog({
        canSelectFiles: false, canSelectFolders: true, canSelectMany: false,
        defaultUri: current ? vscode.Uri.file(current) : vscode.workspace.workspaceFolders?.[0]?.uri,
        openLabel: 'Select TeX Root'
      });
      if (selected?.[0]) await update(pathName, selected[0].fsPath);
      return;
    }
    if (pathName === 'compilation.mainFilePath') {
      const root = vscode.workspace.workspaceFolders?.[0];
      if (!root) return void vscode.window.showWarningMessage('Open a workspace before selecting the main TeX file.');
      const current = String(configuration().get(pathName, '') || '');
      const selected = await vscode.window.showOpenDialog({
        canSelectFiles: true, canSelectFolders: false, canSelectMany: false,
        defaultUri: current ? vscode.Uri.file(path.resolve(root.uri.fsPath, current)) : root.uri,
        filters: { 'TeX files': ['tex', 'mkxl', 'mkiv', 'mkvi', 'mklx'] }, openLabel: 'Select Main File'
      });
      if (!selected?.[0]) return;
      const relative = path.relative(root.uri.fsPath, selected[0].fsPath);
      if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
        return void vscode.window.showWarningMessage('The main TeX file must be inside the first workspace folder.');
      }
      await update(pathName, relative.split(path.sep).join('/'));
      return;
    }
    const current = configuration().get(pathName, setting.default);
    const value = await vscode.window.showInputBox({
      title: setting.title,
      prompt: setting.description,
      value: String(current ?? ''),
      ignoreFocusOut: true
    });
    if (value !== undefined) await update(pathName, value);
  }

  async function editFontSize(name) {
    const setting = fontSettings.get('fontSwitches.fontSizes');
    const entries = Object.fromEntries(configurationObjectEntries(setting));
    const value = await vscode.window.showInputBox({
      title: 'Edit Font Size: ' + name,
      prompt: 'ConTeXt command template; use ${text} for the selected text.',
      value: entries[name] || '',
      ignoreFocusOut: true
    });
    if (value !== undefined) {
      entries[name] = value;
      await updateFontSizes(Object.entries(entries));
    }
  }

  async function addFontSize() {
    const setting = fontSettings.get('fontSwitches.fontSizes');
    const entries = Object.fromEntries(configurationObjectEntries(setting));
    const name = await vscode.window.showInputBox({ title: 'Add Font Size', prompt: 'Name shown in the Font Size list.' });
    if (!name?.trim()) return;
    if (Object.prototype.hasOwnProperty.call(entries, name.trim())) {
      return void vscode.window.showWarningMessage(`Font size “${name.trim()}” already exists.`);
    }
    const template = await vscode.window.showInputBox({
      title: 'Command for ' + name.trim(),
      prompt: 'ConTeXt command template; use ${text} for the selected text.',
      value: '{\\tfa ${text}}'
    });
    if (template !== undefined) {
      entries[name.trim()] = template;
      await updateFontSizes(Object.entries(entries));
    }
  }

  async function removeFontSize(item) {
    const setting = fontSettings.get('fontSwitches.fontSizes');
    const entries = Object.fromEntries(configurationObjectEntries(setting));
    if (!item || !Object.prototype.hasOwnProperty.call(entries, item.name)) return;
    delete entries[item.name];
    await updateFontSizes(Object.entries(entries));
  }

  async function resetSetting(item) {
    if (!item) return;
    const paths = item.resetPaths || [item.path].filter(Boolean);
    const resetFontSizes = paths.includes('fontSwitches.fontSizes');
    const storage = fontSizeOrderStorage();
    const previousOrder = storage.get(FONT_SIZE_ORDER_KEY);
    if (resetFontSizes) await storage.update(FONT_SIZE_ORDER_KEY, undefined);
    try {
      for (const settingPath of paths) {
        const setting = schemas.get(settingPath);
        if (setting) await configuration().update(settingPath, undefined, configTarget(setting));
      }
    } catch (error) {
      if (resetFontSizes) await storage.update(FONT_SIZE_ORDER_KEY, previousOrder);
      throw error;
    }
    changed.fire(undefined);
  }

  const view = vscode.window.createTreeView(VIEW_ID, {
    treeDataProvider: provider,
    showCollapseAll: true,
    manageCheckboxStateManually: true,
    canSelectMany: true,
    dragAndDropController: provider
  });
  tree = view;
  view.description = target === 'workspace' ? 'Workspace settings' : 'User settings';
  context.subscriptions.push(
    changed,
    view,
    view.onDidChangeCheckboxState(event => {
      writeQueue = writeQueue.then(async () => {
        for (const [node, state] of event.items) {
          if (node.checkboxPath) {
            const setting = schemas.get(node.checkboxPath);
            if (setting) await configuration().update(node.checkboxPath, state === vscode.TreeItemCheckboxState.Checked, configTarget(setting));
          }
        }
        changed.fire(undefined);
      }).catch(error => vscode.window.showErrorMessage(`Could not update setting: ${error.message || error}`));
    }),
    vscode.workspace.onDidChangeConfiguration(event => {
      if (event.affectsConfiguration(SECTION)) changed.fire(undefined);
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      if (!vscode.workspace.workspaceFolders?.length) target = 'user';
      updateTargetDescription();
      changed.fire(undefined);
    }),
    vscode.commands.registerCommand('contextIntellisense.editSetting', item =>
      editSetting(typeof item === 'string' ? item : item?.editPath || item?.path)),
    vscode.commands.registerCommand('contextIntellisense.editFontSize', item =>
      editFontSize(typeof item === 'string' ? item : item?.name)),
    vscode.commands.registerCommand('contextIntellisense.removeFontSize', removeFontSize),
    vscode.commands.registerCommand('contextIntellisense.addFontSize', addFontSize),
    vscode.commands.registerCommand('contextIntellisense.resetSetting', resetSetting),
    vscode.commands.registerCommand('contextIntellisense.chooseSettingsTarget', chooseTarget)
  );

  async function chooseTarget() {
    const choices = [{ label: 'User Settings', value: 'user' }];
    if (vscode.workspace.workspaceFolders?.length) choices.push({ label: 'Workspace Settings', value: 'workspace' });
    const selection = await vscode.window.showQuickPick(choices, {
      placeHolder: 'Choose where edits from this panel are saved',
      title: 'Settings Target'
    });
    if (selection) {
      target = selection.value;
      await context.globalState.update('contextIntellisense.settingsTarget', target);
      updateTargetDescription();
    }
  }

  function updateTargetDescription() {
    if (!vscode.workspace.workspaceFolders?.length && target === 'workspace') target = 'user';
    tree.description = target === 'workspace' ? 'Workspace settings' : 'User settings';
  }
};

function settingTitle(id) {
  const names = {
    'contextIntellisense.distribution.texRootPath': 'TeX Root Path',
    'contextIntellisense.compilation.mainFilePath': 'Main TeX File',
    'contextIntellisense.compilation.openPdfAfterCompile': 'Open PDF After Compile',
    'contextIntellisense.explorer.hideTemporaryFiles': 'Hide Temporary Files',
    'contextIntellisense.synctex.mode': 'Forward Search Trigger',
    'contextIntellisense.synctex.viewerMode': 'PDF Viewer Integration',
    'contextIntellisense.math.hover.enabled': 'Hover',
    'contextIntellisense.math.editor.enabled': 'Editor',
    'contextIntellisense.fontSwitches.bold': 'Bold Command',
    'contextIntellisense.fontSwitches.emphasize': 'Emphasize Command',
    'contextIntellisense.fontSwitches.fontSizes': 'Font Sizes',
    'contextIntellisense.fontSwitches.color': 'Color Command'
  };
  if (names[id]) return names[id];
  return id.split('.').pop().replace(/([a-z])([A-Z])/g, '$1 $2').replace(/^./, char => char.toUpperCase());
}
