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
        () => pickProcess(context, false)));
    context.subscriptions.push(commands.registerCommand('lldb.pickMyProcess',
        () => pickProcess(context, true)));
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

async function pickProcess(context: ExtensionContext, currentUserOnly: boolean): Promise<number> {
    let is_windows = process.platform == 'win32';
    var command: string;
    if (!is_windows) {
        if (currentUserOnly)
            command = 'ps x';
        else
            command = 'ps ax';
    } else {
        if (currentUserOnly)
            command = 'tasklist /V /FO CSV /FI "USERNAME eq ' + process.env['USERNAME'] + '"';
        else
            command = 'tasklist /V /FO CSV';
    }
    let stdout = await new Promise<string>((resolve, reject) => {
        cp.exec(command, (error, stdout, stderr) => {
            if (error) reject(error);
            else resolve(stdout)
        })
    });
    let lines = stdout.split('\n');
    let items: (QuickPickItem & { pid: number })[] = [];

    var re: RegExp, idx: number[];
    if (!is_windows) {
        re = /^\s*(\d+)\s+.*?\s+.*?\s+.*?\s+(.*)()$/;
        idx = [1, 2, 3];
    } else {
        // name, pid, ..., window title
        re = /^"([^"]*)","([^"]*)",(?:"[^"]*",){6}"([^"]*)"/;
        idx = [2, 1, 3];
    }
    for (var i = 1; i < lines.length; ++i) {
        let groups = re.exec(lines[i]);
        if (groups) {
            let pid = parseInt(groups[idx[0]]);
            let name = groups[idx[1]];
            let descr = groups[idx[2]];
            let item = { label: format('%d: %s', pid, name), description: descr, pid: pid };
            items.unshift(item);
        }
    }
    let item = await window.showQuickPick(items);
    if (item) {
        return item.pid;
    } else {
        throw Error('Cancelled');
    }
}