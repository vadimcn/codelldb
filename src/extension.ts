'use strict';
import {workspace, languages, window, commands, ExtensionContext, Disposable} from 'vscode';
import {withSession} from './adapterSession';

export function activate(context: ExtensionContext) {
    context.subscriptions.push(commands.registerCommand('lldb.showDisassembly',
        () => showDisassembly(context)));
    context.subscriptions.push(commands.registerCommand('lldb.toggleDisassembly',
        () => toggleDisassembly(context)));
    context.subscriptions.push(commands.registerCommand('lldb.displayFormat',
        () => displayFormat(context)));
}

async function showDisassembly(context: ExtensionContext) {
    let selection = await window.showQuickPick(['always', 'auto', 'never']);
    withSession(session => session.send('showDisassembly', { value: selection }));
}

async function toggleDisassembly(context: ExtensionContext) {
    withSession(session => session.send('showDisassembly', { value: 'toggle' }));
}

async function displayFormat(context: ExtensionContext) {
    let selection = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
    withSession(session => session.send('displayFormat', { value: selection }));
}