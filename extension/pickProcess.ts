import { window, workspace, QuickPickItem, Uri, ExtensionContext } from 'vscode';
import * as path from 'path';
import * as os from 'os';
import * as cp from 'child_process';
import * as adapter from './novsc/adapter';
import * as async from './novsc/async';

type ProcessItem = QuickPickItem & { pid: number };

export async function pickProcess(context: ExtensionContext, allUsers: boolean): Promise<string> {
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
            qpick.items = await getProcessList(context, allUsers);
            qpick.busy = false;
        });
        qpick.onDidHide(() => {
            resolve(undefined);
            qpick.dispose();
        });
        qpick.matchOnDetail = true;
        qpick.matchOnDescription = true;
        qpick.busy = true;
        qpick.show();
        qpick.items = await getProcessList(context, allUsers);
        qpick.busy = false;
    });
}

async function getProcessList(context: ExtensionContext, allUsers: boolean): Promise<ProcessItem[]> {
    let lldb = os.platform() != 'win32' ? 'lldb' : 'lldb.exe';
    let lldbPath = path.join(context.extensionPath, 'lldb', 'bin', lldb);
    if (!await async.fs.exists(lldbPath)) {
        lldbPath = lldb;
    }
    let folder = workspace.workspaceFolders[0];
    let config = workspace.getConfiguration('lldb', folder?.uri);
    let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));
    let lldbCommand = 'platform process list --show-args';
    if (allUsers)
        lldbCommand += ' --all-users';
    let command = `${lldbPath} --batch --no-lldbinit --one-line "${lldbCommand}"`;

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
            return parseProcessEntries(lines.slice(i + 2), argsOffset);
        }
    }
    return [];
}

function parseProcessEntries(lines: string[], argsOffset: number): ProcessItem[] {
    let items = [];
    for (let line of lines) {
        // Process items always start with two integers (pid and ppid); otherwise, we assume that the line
        // is a continuation of the previous process's argument list caused by an embedded newline character.
        let matches = line.match(/^(\d+)\s+(\d+)\s+/);
        if (matches != null) {
            let pid = parseInt(matches[1]);
            let args = line.substring(argsOffset).trim();
            items.push({ label: `${pid}`, description: args, pid: pid });
            continue;
        }
        // Continuation
        items[items.length - 1].description += '\n' + line;
    }
    return items;
}
