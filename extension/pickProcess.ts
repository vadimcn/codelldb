import { window, workspace, QuickPickItem, Uri, ExtensionContext } from 'vscode';
import * as path from 'path';
import * as os from 'os';
import * as cp from 'child_process';
import * as adapter from './novsc/adapter';
import * as async from './novsc/async';
import { getExtensionConfig } from './main';

type ProcessItem = QuickPickItem & { pid: number };

// Options to pickProcess when it is used as part of an input variable
type PickProcessOptions = { initCommands: string[], filter: string };

export async function pickProcess(context: ExtensionContext, allUsers: boolean, options: PickProcessOptions): Promise<string> {
    return new Promise<string>(async (resolve) => {
        let showingAll = {
            iconPath: Uri.file(context.extensionPath + '/images/users.svg'),
            tooltip: 'Showing all processes'
        };
        let showingMy = {
            iconPath: Uri.file(context.extensionPath + '/images/user.svg'),
            tooltip: 'Showing own processes'
        };

        let qpick = window.createQuickPick<ProcessItem>();
        qpick.title = 'Select a process:';
        qpick.buttons = [allUsers ? showingAll : showingMy];
        qpick.matchOnDetail = true;
        qpick.matchOnDescription = true;
        qpick.ignoreFocusOut = true;

        qpick.onDidAccept(() => {
            if (qpick.selectedItems && qpick.selectedItems[0])
                resolve(qpick.selectedItems[0].pid.toString())
            else
                resolve(undefined);
            qpick.dispose();
        });

        qpick.onDidTriggerButton(async () => {
            allUsers = !allUsers;
            qpick.buttons = [allUsers ? showingAll : showingMy];
            qpick.busy = true;
            try {
                qpick.items = await getProcessList(context, allUsers, options);
                qpick.busy = false;
            } catch (e) {
                qpick.dispose();
                window.showErrorMessage(e?.toString() || 'Unknown error getting process list.');
            }
        });

        qpick.onDidHide(() => {
            resolve(undefined);
            qpick.dispose();
        });

        qpick.busy = true;
        qpick.show();
        try {
            qpick.items = await getProcessList(context, allUsers, options);
            qpick.busy = false;
        } catch (e) {
            qpick.dispose();
            window.showErrorMessage(e?.toString() || 'Unknown error getting process list.');
        }
    });
}

async function getProcessList(context: ExtensionContext, allUsers: boolean, options: PickProcessOptions): Promise<ProcessItem[]> {
    let lldb = os.platform() != 'win32' ? 'lldb' : 'lldb.exe';
    let lldbPath = path.join(context.extensionPath, 'lldb', 'bin', lldb);
    if (!await async.fs.exists(lldbPath)) {
        lldbPath = lldb;
    }
    let folder = workspace.workspaceFolders?.[0];
    let config = getExtensionConfig(folder?.uri);
    let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));

    let initArgs = '';
    if (Array.isArray(options.initCommands)) {
        options.initCommands.forEach((command: string) => {
            initArgs += ` --one-line "${command}"`;
        });
    }

    let processListCommand = 'platform process list --show-args';
    if (allUsers)
        processListCommand += ' --all-users';

    let command = `${lldbPath} --batch --no-lldbinit ${initArgs} --one-line "${processListCommand}"`;

    let stdout = await new Promise<string>((resolve, reject) => {
        cp.exec(command, { env: env }, (error, stdout) => {
            if (error) reject(error);
            else resolve(stdout)
        })
    });

    // A typical output will look like this:
    //
    // 224 matching processes were found on "host"
    // PID    PARENT USER       TRIPLE                         ARGUMENTS
    // ====== ====== ========== ============================== ============================
    // 9756   1      user       x86_64-pc-linux-gnu            /lib/systemd/systemd --user
    // ...
    let lines = stdout.split('\n');
    for (let i = 0; i < lines.length; ++i) {
        let argsOffset = lines[i].indexOf('ARGUMENTS')
        if (argsOffset > 0) {
            return parseProcessEntries(lines.slice(i + 2), argsOffset, options.filter);
        }
    }
    return [];
}

function parseProcessEntries(lines: string[], argsOffset: number, filter: string): ProcessItem[] {
    let filterRegExp = filter ? new RegExp(filter) : undefined;
    let items = [];
    for (let line of lines) {
        // Process items always start with two integers (pid and ppid); otherwise, we assume that the line
        // is a continuation of the previous process's argument list caused by an embedded newline character.
        let matches = line.match(/^(\d+)\s+(\d+)\s+/);
        if (matches != null) {
            let pid = parseInt(matches[1]);
            let args = line.substring(argsOffset).trim();
            if (filterRegExp && !filterRegExp.test(args)) {
                continue;
            }
            items.push({ label: `${pid}`, description: args, pid: pid });
            continue;
        }
        if (line) {
            // Continuation
            items[items.length - 1].description += '\n' + line;
        }
    }
    if (!items.length) {
        if (filter) {
          throw `No processes found matching: ${filter}`;
        }
        else {
          throw `Unable to find processes in:\n${lines.join('\n')}`;
        }
    }
    return items;
}
