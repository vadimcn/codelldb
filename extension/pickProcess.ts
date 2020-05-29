import { QuickPickItem, window, Uri, ExtensionContext } from 'vscode';
import * as cp from 'child_process';

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
    let command: string;
    let matchLine;

    if (process.platform != 'win32') {
        if (allUsers)
            command = 'ps ax';
        else
            command = 'ps x';

        let regex = /^\s*(\d+)\s+.*?\s+.*?\s+.*?\s+(.*)()$/;
        matchLine = (line: string) => {
            let groups = regex.exec(line);
            if (!groups)
                return null;
            let item = {
                pid: parseInt(groups[1]),
                name: groups[2],
                description: groups[3]
            };
            // Filter out kernel threads
            if (item.name.startsWith('[') && item.name.endsWith(']'))
                return null;
            return item;
        };
    } else {
        if (allUsers)
            command = 'tasklist /V /FO CSV';
        else
            command = 'tasklist /V /FO CSV /FI "USERNAME eq ' + process.env['USERNAME'] + '"';
        // name, pid, ..., window title
        let regex = /^"([^"]*)","([^"]*)",(?:"[^"]*",){6}"([^"]*)"/;
        matchLine = (line: string) => {
            let groups = regex.exec(line);
            return groups == null ? null : {
                pid: parseInt(groups[2]),
                name: groups[1],
                description: groups[3]
            };
        };
    }
    let stdout = await new Promise<string>((resolve, reject) => {
        cp.exec(command, (error, stdout) => {
            if (error) reject(error);
            else resolve(stdout)
        })
    });
    let lines = stdout.split('\n');
    let items = [];
    for (let i = 1; i < lines.length; ++i) {
        let item = matchLine(lines[i]);
        if (item) {
            items.push({ label: `${item.pid}: ${item.name}`, description: item.description, pid: item.pid });
        }
    }
    return items;
}
