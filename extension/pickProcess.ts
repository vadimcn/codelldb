import { window, workspace, QuickPickItem, Uri, ExtensionContext, ThemeIcon } from 'vscode';
import * as path from 'path';
import * as os from 'os';
import * as cp from 'child_process';
import * as adapter from './novsc/adapter';
import * as async from './novsc/async';
import { getExtensionConfig } from './main';
import { pid } from 'process';

type PickProcessOptions = { initCommands: string[], filter: string };

type ProcessItem = QuickPickItem & { pid: number };

class ProcessInfo {
    pid: number;
    ppid: number;
    user: string;
    triple: string;
    command: string;
}

export async function pickProcess(context: ExtensionContext, allUsers: boolean, options: PickProcessOptions): Promise<string> {
    return new Promise<string>(async (resolve, reject) => {
        try {
            let showingAllButton = {
                iconPath: Uri.file(context.extensionPath + '/images/users.svg'),
                tooltip: 'Showing all processes'
            };
            let showingMyButton = {
                iconPath: Uri.file(context.extensionPath + '/images/user.svg'),
                tooltip: 'Showing owned processes'
            };
            let detailsButton = {
                iconPath: new ThemeIcon('list-flat'),
                tooltip: 'Toggle details'
            }

            let showDetails = false;
            let processes = filterProcesses(await getProcessList(context, allUsers, options), options);

            let qpick = window.createQuickPick<ProcessItem>();
            qpick.title = 'Select a process:';
            qpick.buttons = [allUsers ? showingAllButton : showingMyButton, detailsButton];
            qpick.matchOnDescription = true;
            qpick.matchOnDetail = true;
            qpick.ignoreFocusOut = true;

            qpick.onDidAccept(() => {
                if (qpick.selectedItems && qpick.selectedItems[0])
                    resolve(qpick.selectedItems[0].pid.toString())
                else
                    resolve(undefined);
                qpick.dispose();
            });

            qpick.onDidTriggerButton(async (button) => {
                if (button == qpick.buttons[0]) {
                    allUsers = !allUsers;
                    processes = filterProcesses(await getProcessList(context, allUsers, options), options);
                } else if (button == qpick.buttons[1]) {
                    showDetails = !showDetails;
                }
                qpick.buttons = [allUsers ? showingAllButton : showingMyButton, detailsButton];
                qpick.busy = true;
                qpick.items = processes.map(process => processIntoToItem(process, showDetails));
                qpick.busy = false;
            });

            qpick.onDidHide(() => {
                resolve(undefined);
                qpick.dispose();
            });

            qpick.busy = true;
            qpick.show();
            qpick.items = processes.map(process => processIntoToItem(process, showDetails));
            qpick.busy = false;
        }
        catch (e) {
            reject(e);
        }
    });
}

function processIntoToItem(process: ProcessInfo, details: boolean): ProcessItem {
    let item: ProcessItem = {
        pid: process.pid,
        label: `${process.pid}`,
        description: process.command,
    };
    if (details) {
        item.detail = `PPID: ${process.ppid}, User: ${process.user}, Triple: ${process.triple}`;
    }
    return item;
}

function filterProcesses(processes: ProcessInfo[], options: PickProcessOptions): ProcessInfo[] {
    if (options && options.filter) {
        let re = new RegExp(options.filter);
        processes = processes.filter(p => re.test(p.command));
    }
    return processes;
}


async function getProcessList(context: ExtensionContext, allUsers: boolean, options: PickProcessOptions): Promise<ProcessInfo[]> {
    let lldb = os.platform() != 'win32' ? 'lldb' : 'lldb.exe';
    let lldbPath = path.join(context.extensionPath, 'lldb', 'bin', lldb);
    if (!await async.fs.exists(lldbPath)) {
        lldbPath = lldb; // Search PATH
    }
    let folder = workspace.workspaceFolders?.[0];
    let config = getExtensionConfig(folder?.uri);
    let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));

    let initArgs = '';
    if (options && Array.isArray(options.initCommands)) {
        for (let command of options.initCommands) {
            initArgs += ` --one-line "${command}"`;
        }
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
        if (lines[i].indexOf('ARGUMENTS') > 0) {
            let colOffsets = {
                parent: lines[i].indexOf('PARENT'),
                user: lines[i].indexOf('USER'),
                triple: lines[i].indexOf('TRIPLE'),
                args: lines[i].indexOf('ARGUMENTS')
            };
            return parseProcessEntries(lines.slice(i + 2), colOffsets);
        }
    }
    return [];
}

function parseProcessEntries(lines: string[], colOffsets: any): ProcessInfo[] {
    let items = [];
    for (let line of lines) {
        // Process items always start with two integers (pid and ppid); otherwise, we assume that the line
        // is a continuation of the previous process's argument list caused by an embedded newline character.
        let matches = line.match(/^(\d+)\s+(\d+)\s+/);
        if (matches != null) {
            let process = new ProcessInfo();
            process.pid = parseInt(matches[1]);
            process.ppid = parseInt(matches[2]);
            process.user = line.substring(colOffsets.user, colOffsets.triple).trim();
            process.triple = line.substring(colOffsets.triple, colOffsets.args).trim();
            process.command = line.substring(colOffsets.args).trim();
            items.push(process);
        } else if (line) {
            // Continuation
            items[items.length - 1].command += '\n' + line;
        }
    }
    return items;
}
