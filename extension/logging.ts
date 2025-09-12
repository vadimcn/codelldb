import { window } from 'vscode';

export let output = window.createOutputChannel('LLDB', 'log');

export async function showErrorWithLog(message: string) {
    let result = await window.showErrorMessage(message, 'Show log');
    if (result != undefined) {
        output.show();
    }
}
