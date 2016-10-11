'use strict';
import { workspace, languages, window, commands, ExtensionContext, Disposable, QuickPickItem } from 'vscode';
import { withSession } from './adapterSession';
import { format } from 'util';
import * as path from 'path';
import * as cp from 'child_process';

export function activate(context: ExtensionContext) {
    context.subscriptions.push(commands.registerCommand('lldb.showDisassembly',
        () => showDisassembly(context)));
    context.subscriptions.push(commands.registerCommand('lldb.toggleDisassembly',
        () => toggleDisassembly(context)));
    context.subscriptions.push(commands.registerCommand('lldb.displayFormat',
        () => displayFormat(context)));
    context.subscriptions.push(commands.registerCommand('lldb.launchDebugServer',
        () => launchDebugServer(context)));
    context.subscriptions.push(commands.registerCommand('lldb.pickProcess',
        () => pickProcess(context)));
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

async function launchDebugServer(context: ExtensionContext) {
    let terminal = window.createTerminal('LLDB Debug Server');
    terminal.sendText('cd ' + context.extensionPath + '\n');
    terminal.sendText('lldb -b -O "script import adapter; adapter.run_tcp_server()"\n');
}

async function pickProcess(context: ExtensionContext): Promise<number> {
    let stdout = await new Promise<string>((resolve) =>
        cp.exec('ps ax', (error, stdout, stderr) => resolve(stdout)));
    let lines = stdout.split('\n');
    let items: (QuickPickItem & { pid: number})[] = [];
    let re = /^\s*(\d+)\s+.*?\s+.*?\s+.*?\s+(.*)$/;
    for (var i = 1; i < lines.length; ++i) {
        let groups = re.exec(lines[i]);
        if (groups) {
            let pid = parseInt(groups[1]);
            let item = { label: format('%d: %s', pid, groups[2]), description: '', pid: pid };
            items.unshift(item);
        }
    }
    let item = await window.showQuickPick(items);
    if (item) {
        return item.pid;
    } else {
        return 0;
    }
}