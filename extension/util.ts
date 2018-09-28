import { DebugConfiguration } from 'vscode';
import * as cp from 'child_process';
import { format } from 'util';
import { QuickPickItem, WorkspaceConfiguration } from 'vscode';
import { Dict } from './extension';

let expandVarRegex = /\$\{(?:([^:}]+):)?([^}]+)\}/g;

export function expandVariables(str: string | String, expander: (type: string, key: string) => string): string {
    let result = str.replace(expandVarRegex, (all: string, type: string, key: string): string => {
        let replacement = expander(type, key);
        return replacement != null ? replacement : all;
    });
    return result;
}

export function expandVariablesInObject(obj: any, expander: (type: string, key: string) => string): any {
    if (typeof obj == 'string' || obj instanceof String)
        return expandVariables(obj, expander);

    if (isScalarValue(obj))
        return obj;

    if (obj instanceof Array)
        return obj.map(v => expandVariablesInObject(v, expander));

    for (var prop of Object.keys(obj))
        obj[prop] = expandVariablesInObject(obj[prop], expander)
    return obj;
}

// Expands variable references of the form ${dbgconfig:name} in all properties of launch configuration.
export function expandDbgConfig(launchConfig: DebugConfiguration, dbgconfigConfig: WorkspaceConfiguration): DebugConfiguration {
    let dbgconfig: Dict<any> = Object.assign({}, dbgconfigConfig);

    // Compute fixed-point of expansion of dbgconfig properties.
    var expanding = '';
    var converged = true;
    let expander = (type: string, key: string) => {
        if (type == 'dbgconfig') {
            if (key == expanding)
                throw new Error('Circular dependency detected during expansion of dbgconfig:' + key);
            let value = dbgconfig[key];
            if (value == undefined)
                throw new Error('dbgconfig:' + key + ' is not defined');
            converged = false;
            return value.toString();
        }
        return null;
    };
    do {
        converged = true;
        for (var prop of Object.keys(dbgconfig)) {
            expanding = prop;
            dbgconfig[prop] = expandVariablesInObject(dbgconfig[prop], expander);
        }
    } while (!converged);

    // Now expand dbgconfigs in the launch configuration.
    launchConfig = expandVariablesInObject(launchConfig, (type, key) => {
        if (type == 'dbgconfig') {
            let value = dbgconfig[key];
            if (value == undefined)
                throw new Error('dbgconfig:' + key + ' is not defined');
            return value.toString();
        }
        return null;
    });
    return launchConfig;
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
    var value = x.workspaceFolderValue;
    if (value === undefined)
        value = x.workspaceValue;
    if (value === undefined)
        value = x.globalValue;
    return value;
}

export function isEmpty(obj: any): boolean {
    if (obj === null || obj === undefined)
        return true;
    if (typeof obj == 'number' || obj instanceof Number)
        return false;
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
    if (isScalarValue(value1))
        return value2;
    // Concatenate arrays.
    if (value1 instanceof Array && value2 instanceof Array)
        return value1.concat(value2);
    // Merge dictionaries.
    return Object.assign({}, value1, value2);
}

function isScalarValue(value: any) {
    return value === null || value === undefined ||
        typeof value == 'boolean' || value instanceof Boolean ||
        typeof value == 'number' || value instanceof Number ||
        typeof value == 'string' || value instanceof String;
}
