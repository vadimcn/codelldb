'use strict';
import * as cp from 'child_process';
import { format } from 'util';
import { QuickPickItem, WorkspaceConfiguration } from 'vscode';

let expandVarRegex = /\$\{(?:([^:}]+):)?([^}]+)\}/g;

export function expandVariables(str: string, expander: (type: string, key: string) => string): string {
    let result = str.replace(expandVarRegex, (all: string, type: string, key: string): string => {
        return expander(type, key);
    });
    return result;
}

export async function getProcessList(currentUserOnly: boolean):
    Promise<(QuickPickItem & { pid: number })[]> {

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
    let items = [];

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
    return items;
}

export function getConfigNoDefault(config: WorkspaceConfiguration, key: string): any {
    let x = config.inspect(key);
    var value = x.globalValue;
    if (value === undefined)
        value = x.workspaceValue;
    if (value === undefined)
        value = x.workspaceFolderValue;
    return value;
}

export function isEmpty(obj: any): boolean {
    if (obj === null || obj === undefined)
        return true;
    if (typeof obj == 'string' || obj instanceof String)
        return obj.length == 0;
    if (obj instanceof Array)
        return obj.length == 0;
    return Object.keys(obj).length == 0;
}

export function mergeValues(value1: any, value2: any): any {
    if (value2 === undefined)
        return value1;
    // For non-container types, value2 wins.
    if (value1 === null || value1 === undefined ||
        typeof value1 == 'boolean' || value1 instanceof Boolean ||
        typeof value1 == 'number' || value1 instanceof Number ||
        typeof value1 == 'string' || value1 instanceof String)
        return value2;
    // Concatenate arrays.
    if (value1 instanceof Array && value2 instanceof Array)
        return value1.concat(value2);
    // Merge dictionaries.
    return Object.assign({}, value1, value2);
}
