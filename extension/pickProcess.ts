import { window, workspace, QuickPickItem, Uri, ExtensionContext } from 'vscode';
import * as path from 'path';
import * as os from 'os';
import * as cp from 'child_process';
import * as adapter from './novsc/adapter';

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
    let lines = stdout.split('\n');

    let items = [];
    for (let i = 0; i < lines.length; ++i) {
        let argsOffset = lines[i].indexOf('ARGUMENTS')
        if (argsOffset > 0) {
            for (let line of lines.slice(i + 2)) {
                let pid = parseInt(line);
                let args = line.substring(argsOffset).trim();
                if (args.length > 0) {
                    items.push({ label: `${pid}`, description: args, pid: pid });
                }
            }
            break;
        }
    }
    return items;
}
