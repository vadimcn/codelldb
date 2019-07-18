import { QuickPickItem, WorkspaceConfiguration, DebugConfiguration, OutputChannel } from 'vscode';
import * as cp from 'child_process';
import * as async from './novsc/async';
import { Dict } from './novsc/commonTypes';
import {expandVariablesInObject } from './novsc/expand';

// Expands variable references of the form ${dbgconfig:name} in all properties of launch configuration.
export function expandDbgConfig(debugConfig: DebugConfiguration, dbgconfigConfig: WorkspaceConfiguration): DebugConfiguration {
    let dbgconfig: Dict<any> = Object.assign({}, dbgconfigConfig);

    // Compute fixed-point of expansion of dbgconfig properties.
    let expanding = '';
    let converged = true;
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
        for (let prop of Object.keys(dbgconfig)) {
            expanding = prop;
            dbgconfig[prop] = expandVariablesInObject(dbgconfig[prop], expander);
        }
    } while (!converged);

    // Now expand dbgconfigs in the launch configuration.
    debugConfig = expandVariablesInObject(debugConfig, (type, key) => {
        if (type == 'dbgconfig') {
            let value = dbgconfig[key];
            if (value == undefined)
                throw new Error('dbgconfig:' + key + ' is not defined');
            return value.toString();
        }
        return null;
    });
    return debugConfig;
}

export function getConfigNoDefault(config: WorkspaceConfiguration, key: string): any {
    let x = config.inspect(key);
    let value = x.workspaceFolderValue;
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

export function logProcessOutput(process: cp.ChildProcess, output: OutputChannel) {
    process.stdout.on('data', chunk => {
        output.append(chunk.toString());
    });
    process.stderr.on('data', chunk => {
        output.append(chunk.toString());
    });
}

export function setIfDefined(target: Dict<any>, config: WorkspaceConfiguration, key: string) {
    let value = getConfigNoDefault(config, key);
    if (value !== undefined)
        target[key] = value;
}


export interface LLDBDirectories {
    shlibDir: string;
    supportExeDir: string;
    pythonDir: string
}

export async function getLLDBDirectories(executable: string): Promise<LLDBDirectories> {
    let statements = [];
    for (let type of ['ePathTypeLLDBShlibDir', 'ePathTypeSupportExecutableDir', 'ePathTypePythonDir']) {
        statements.push(`print('<!' + lldb.SBHostOS.GetLLDBPath(lldb.${type}).fullpath + '!>')`);
    }
    let args = ['-b', '-O', `script ${statements.join(';')}`];
    let { stdout, stderr } = await async.cp.execFile(executable, args);
    let m = (/^<!([^!]*)!>$[^.]*^<!([^!]*)!>[^.]*^<!([^!]*)!>/m).exec(stdout);
    if (m) {
        return {
            shlibDir: m[1],
            supportExeDir: m[2],
            pythonDir: m[3]
        };
    } else {
        throw new Error(stderr);
    }
}

