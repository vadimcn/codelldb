'use strict';

import {
    workspace, languages, window, commands,
    ExtensionContext, Disposable, QuickPickItem, Uri, Event, EventEmitter, OutputChannel
} from 'vscode';
import { format, inspect } from 'util';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as ver from './ver';
import * as util from './util';

let output = window.createOutputChannel('LLDB');

export class AdapterProcess {
    public isAlive: boolean;
    public port: number;

    constructor(process: cp.ChildProcess) {
        this.process = process;
        this.isAlive = true;
        process.on('exit', (code, signal) => {
            this.isAlive = false;
            if (signal) {
                output.appendLine(format('Adapter teminated by %s signal.', signal));
            }
            if (code) {
                output.appendLine(format('Adapter exit code: %d.', code));
            }
        });
    }
    public terminate() {
        if (this.isAlive) {
            this.process.kill();
        }
    }
    process: cp.ChildProcess;
}

// Start debug adapter in TCP session mode and return the port number it is listening on.
export async function startDebugAdapter(context: ExtensionContext): Promise<AdapterProcess> {
    let adapterPath = path.join(context.extensionPath, 'adapter');
    let params = getAdapterParameters();
    let args = ['-b', '-Q',
        '-O', format('command script import \'%s\'', adapterPath),
        '-O', format('script adapter.main.run_tcp_session(0, \'%s\')', params)
    ];
    let lldb = spawnDebugger(args);
    let regex = new RegExp('^Listening on port (\\d+)\\s', 'm');
    let match = await waitPattern(lldb, regex);

    let adapter = new AdapterProcess(lldb);
    adapter.port = parseInt(match[1]);
    return adapter;
}

function getAdapterParameters(): string {
    let config = workspace.getConfiguration('lldb');
    let params = config.get('parameters');
    return new Buffer(JSON.stringify(params)).toString('base64');
}

enum DiagnosticsStatus {
    Succeeded = 0,
    Warning = 1,
    Failed = 2
}

export async function diagnose(): Promise<boolean> {
    let args = ['-b', '-Q',
        '-O', 'script import sys, io, lldb; ' +
        'print(lldb.SBDebugger.Create().GetVersionString()); ' +
        'print("OK")'
    ];
    var status = null;
    try {
        let lldb = spawnDebugger(args);

        var versionPattern = '^lldb version ([0-9.]+)';
        var desiredVersion = '3.9.1';
        if (process.platform.search('win32') != -1) {
            desiredVersion = '4.0.0';
        } else if (process.platform.search('darwin') != -1) {
            versionPattern = '^lldb-([0-9.]+)';
            desiredVersion = '360.1.68';
        }
        let pattern = new RegExp('(?:' + versionPattern + '[^]*)?^OK$', 'm');
        let match = await waitPattern(lldb, pattern);
        status = DiagnosticsStatus.Succeeded;
        let version = match[1];
        if (version && ver.lt(version, desiredVersion)) {
            output.appendLine(
                format('The version of your LLDB has been detected as %s. ' +
                    'For best results please consider upgrading to least %s.',
                    version, desiredVersion));
            status = DiagnosticsStatus.Warning;
        }
        if (process.platform.indexOf('linux') >= 0) {
            status = Math.max(status, checkPTraceScope());
        }
    } catch (err) {
        output.appendLine('---');
        output.appendLine(format('An exception was raised while launching debugger: %s', inspect(err)));
        status = DiagnosticsStatus.Failed;
    }
    output.show(true);
    switch (<number>status) {
        case DiagnosticsStatus.Succeeded:
            window.showInformationMessage('LLDB self-test completed successfuly.');
            break;
        case DiagnosticsStatus.Warning:
            window.showWarningMessage('LLDB self-test completed with warnings.  Please check LLDB output panel for details.');
            break;
        case DiagnosticsStatus.Failed:
            window.showInformationMessage('LLDB self-test FAILED.');
            break;
    }
    return status != DiagnosticsStatus.Failed;
}

function checkPTraceScope(): DiagnosticsStatus {
    let ptraceScopePath = '/proc/sys/kernel/yama/ptrace_scope';
    try {
        let ptraceScope = fs.readFileSync(ptraceScopePath).toString('ascii');
        output.appendLine('The value of \'' + ptraceScopePath + '\' is: ' + ptraceScope);
        let moreInfo = 'For more information see: https://en.wikipedia.org/wiki/Ptrace, https://www.kernel.org/doc/Documentation/security/Yama.txt';
        switch (parseInt(ptraceScope)) {
            case 0:
                return DiagnosticsStatus.Succeeded;
            case 1:
                output.appendLine('Warning: Your system configuration restricts process attach to child processes only.');
                output.appendLine(moreInfo);
                return DiagnosticsStatus.Succeeded; // This is a fairly typical setting, let's not annoy user with warnings.
            case 2:
                output.appendLine('Warning: Your system configuration restricts debugging to privileged processes only.');
                output.appendLine(moreInfo);
                return DiagnosticsStatus.Warning;
            case 3:
                output.appendLine('Warning: Your system configuration has disabled debugging.');
                output.appendLine(moreInfo);
                return DiagnosticsStatus.Warning;
            default:
                output.appendLine('Warning: Unknown value of ptrace_scope.');
                output.appendLine(moreInfo);
                return DiagnosticsStatus.Warning;
        }
    } catch (err) {
        output.appendLine('Couldn\'t read ' + ptraceScopePath + ' : ' + err.message);
        return DiagnosticsStatus.Succeeded; // Ignore
    }
}

// Spawn LLDB with the specified arguments, wait for it to output something matching
// regex pattern, or until the timeout expires.
function spawnDebugger(args: string[]): cp.ChildProcess {
    output.clear();
    let config = workspace.getConfiguration('lldb');
    let lldbPath = config.get('executable', 'lldb');

    let lldbEnv: any = config.get('environment', {});
    let env = Object.assign({}, process.env);
    for (var key in lldbEnv) {
        env[key] = util.expandVariables(lldbEnv[key], (type, key) => {
            if (type == 'env') return process.env[key];
            throw new Error('Unknown variable type ' + type);
        });
    }

    let options = {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: env,
        cwd: workspace.rootPath
    };
    if (process.platform.search('darwin') != -1) {
        // Make sure LLDB finds system Python before Brew Python
        // https://github.com/Homebrew/legacy-homebrew/issues/47201
        options.env['PATH'] = '/usr/bin:' + process.env['PATH'];
    }
    return cp.spawn(lldbPath, args, options);
}

function waitPattern(lldb: cp.ChildProcess, pattern: RegExp, timeout_millis = 5000) {
    return new Promise<RegExpExecArray>((resolve, reject) => {
        var promisePending = true;
        var adapterOutput = '';
        // Wait for expected pattern in stdout.
        lldb.stdout.on('data', (chunk) => {
            let chunkStr = chunk.toString();
            output.append(chunkStr); // Send to "LLDB" output pane.
            if (promisePending) {
                adapterOutput += chunkStr;
                let match = pattern.exec(adapterOutput);
                if (match) {
                    clearTimeout(timer);
                    adapterOutput = null;
                    promisePending = false;
                    resolve(match);
                }
            }
        });
        // Send sdterr to the output pane as well.
        lldb.stderr.on('data', (chunk) => {
            let chunkStr = chunk.toString();
            output.append(chunkStr);
        });
        // On spawn error.
        lldb.on('error', (err) => {
            promisePending = false;
            reject(err);
        });
        // Bail if LLDB does not start within the specified timeout.
        let timer = setTimeout(() => {
            if (promisePending) {
                lldb.kill();
                let err = Error('The debugger did not start within the allotted time.');
                (<any>err).code = 'Timeout';
                (<any>err).stdout = adapterOutput;
                promisePending = false;
                reject(err);
            }
        }, timeout_millis);
        // Premature exit.
        lldb.on('exit', (code, signal) => {
            if (promisePending) {
                let err = Error('The debugger exited without completing startup handshake.');
                (<any>err).code = 'Handshake';
                (<any>err).stdout = adapterOutput;
                promisePending = false;
                reject(err);
            }
        });
    });
}

export async function analyzeStartupError(err: Error) {
    output.appendLine(err.toString());
    output.show(true)
    let e = <any>err;
    let diagnostics = 'Run diagnostics';
    var actionAsync;
    if (e.code == 'ENOENT') {
        actionAsync = window.showErrorMessage(
            format('Could not start debugging because executable \'%s\' was not found.', e.path),
            diagnostics);
    } else if (e.code == 'Timeout' || e.code == 'Handshake') {
        actionAsync = window.showErrorMessage(err.message, diagnostics);
    } else {
        actionAsync = window.showErrorMessage('Could not start debugging.', diagnostics);
    }

    if ((await actionAsync) == diagnostics) {
        await diagnose();
    }
}

export async function getAdapterExecutable(context: ExtensionContext): Promise<any> {
    let config = workspace.getConfiguration('lldb');
    let lldbPath = config.get('executable', 'lldb');
    let adapterPath = path.join(context.extensionPath, 'adapter');
    let params = getAdapterParameters();
    return {
        command: lldbPath,
        args: ['-b', '-Q',
            '-O', format('command script import \'%s\'', adapterPath),
            '-O', format('script adapter.main.run_stdio_session(\'%s\')', params)
        ]
    }
}

export async function launchDebugServer(context: ExtensionContext) {
    let config = workspace.getConfiguration('lldb');
    let lldbPath = config.get('executable', 'lldb');

    let terminal = window.createTerminal('LLDB Debug Server');
    let adapterPath = path.join(context.extensionPath, 'adapter');
    let command =
        format('%s -b -O "command script import \'%s\'" ', lldbPath, adapterPath) +
        format('-O "script adapter.main.run_tcp_server()"\n');
    terminal.sendText(command);
    terminal.show(true);
}
