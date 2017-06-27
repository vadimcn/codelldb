'use strict';

import {
    workspace, languages, window, commands, ExtensionContext, Disposable, QuickPickItem,
    Uri, Event, EventEmitter, OutputChannel
} from 'vscode';
import { format, inspect } from 'util';
import * as cp from 'child_process';
import * as path from 'path';
import * as ver from './ver';

let output = window.createOutputChannel('LLDB');

// Start debug adapter in TCP session mode and return the port number it is listening on.
export async function startDebugAdapter(context: ExtensionContext): Promise<number> {
    let adapterPath = path.join(context.extensionPath, 'adapter');
    let logging = getAdapterLoggingSettings();
    let args = ['-b', '-Q',
        '-O', format('command script import \'%s\'', adapterPath),
        '-O', format('script adapter.main.run_tcp_session(0, %s)', logging)
    ];
    let regex = new RegExp('^Listening on port (\\d+)\\s', 'm');
    let match = await spawnDebugger(args, regex);
    return parseInt(match[1]);
}

function getAdapterLoggingSettings(): string {
    let config = workspace.getConfiguration('lldb');
    let lldbLog = config.get('log', null);
    if (lldbLog) {
        var logPath = 'None';
        var logLevel = 0;
        if (typeof (lldbLog) == 'string') {
            logPath = lldbLog;
        } else {
            logPath = lldbLog.path;
            logLevel = lldbLog.level;
        }
        return format('log_file=\'b64:%s\',log_level=%d',
            new Buffer(logPath).toString('base64'), logLevel);
    } else {
        return '';
    }
}

export async function diagnose(): Promise<boolean> {
    let args = ['-b', '-Q',
        '-O', 'script import sys, io, lldb; ' +
        'print(lldb.SBDebugger.Create().GetVersionString()); ' +
        'print("OK")'
    ];
    var succeeded = false;
    try {
        var versionPattern = '^lldb version ([0-9.]+)';
        var desiredVersion = '3.9.1';
        if (process.platform.search('win32') != -1) {
            desiredVersion = '4.0.0';
        } else if (process.platform.search('darwin') != -1) {
            versionPattern = '^lldb-([0-9.]+)';
            desiredVersion = '360.1.68';
        }
        let pattern = new RegExp('(?:' + versionPattern + '[^]*)?^OK$', 'm');

        let match = await spawnDebugger(args, pattern);
        window.showInformationMessage('LLDB self-test completed successfuly.');
        let version = match[1];
        if (version && ver.lt(version, desiredVersion)) {
            window.showWarningMessage(
                format('The version of your LLDB has been detected as %s. ' +
                    'For best results please consider upgrading to least %s.',
                    version, desiredVersion));
        }
        succeeded = true;
    } catch (err) {
        output.appendLine('---');
        output.appendLine(format('An exception was raised while launching debugger: %s', inspect(err)));
        window.showErrorMessage('LLDB self-test FAILED.');
    }
    output.show(true);
    return succeeded;
}

// Spawn LLDB with the specified arguments, wait for it to output something matching
// regex pattern, or until the timeout expires.
function spawnDebugger(args: string[], pattern: RegExp, timeout_millis = 5000): Promise<RegExpExecArray> {
    output.clear();
    let config = workspace.getConfiguration('lldb');
    let lldbPath = config.get('executable', 'lldb');
    let options = {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: Object.assign({}, process.env),
        cwd: workspace.rootPath
    };
    if (process.platform.search('darwin') != -1) {
        // Make sure LLDB finds system Python before Brew Python
        // https://github.com/Homebrew/legacy-homebrew/issues/47201
        options.env['PATH'] = '/usr/bin:' + process.env['PATH'];
    }
    let lldb = cp.spawn(lldbPath, args, options);
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
    return {
        command: lldbPath,
        args: ['-b', '-Q',
            '-O', format('command script import \'%s\'', adapterPath),
            '-O', format('script adapter.main.run_stdio_session(%s)', getAdapterLoggingSettings())
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
}
