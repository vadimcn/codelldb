import { QuickPickItem, window, Uri, ExtensionContext } from 'vscode';
import * as cp from 'child_process';

type ProcessItem = QuickPickItem & { pid: number };

export async function pickProcess(context: ExtensionContext, allUsers: boolean): Promise<string> {
    return new Promise<string>(async (resolve) => {
        let showingAll = {
            iconPath: Uri.file(context.extensionPath + '/images/checked.svg'),
            tooltip: 'Show processes from all users.'
        };
        let showingMy = {
            iconPath: Uri.file(context.extensionPath + '/images/unchecked.svg'),
            tooltip: 'Show processes from all users.'
        };
        let qpick = window.createQuickPick<ProcessItem>();
        qpick.title = 'Select a process:';
        qpick.items = await getProcessList(allUsers);
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
            qpick.items = await getProcessList(allUsers);
            qpick.buttons = [allUsers ? showingAll : showingMy];
        });
        qpick.onDidHide(() => {
            resolve(undefined);
            qpick.dispose();
        });
        qpick.show();
    });
}

async function getProcessList(allUsers: boolean): Promise<ProcessItem[]> {
    let is_windows = process.platform == 'win32';
    let command: string;
    if (!is_windows) {
        if (allUsers)
            command = 'ps ax';
        else
            command = 'ps x';
    } else {
        if (allUsers)
            command = 'tasklist /V /FO CSV';
        else
            command = 'tasklist /V /FO CSV /FI "USERNAME eq ' + process.env['USERNAME'] + '"';
    }
    let stdout = await new Promise<string>((resolve, reject) => {
        cp.exec(command, (error, stdout) => {
            if (error) reject(error);
            else resolve(stdout)
        })
    });
    let lines = stdout.split('\n');
    let items = [];

    let re: RegExp, idx: number[];
    if (!is_windows) {
        re = /^\s*(\d+)\s+.*?\s+.*?\s+.*?\s+(.*)()$/;
        idx = [1, 2, 3];
    } else {
        // name, pid, ..., window title
        re = /^"([^"]*)","([^"]*)",(?:"[^"]*",){6}"([^"]*)"/;
        idx = [2, 1, 3];
    }
    for (let i = 1; i < lines.length; ++i) {
        let groups = re.exec(lines[i]);
        if (groups) {
            let pid = parseInt(groups[idx[0]]);
            let name = groups[idx[1]];
            let descr = groups[idx[2]];
            let item = { label: `${pid}: ${name}`, description: descr, pid: pid };
            items.push(item);
        }
    }
    return items;
}
