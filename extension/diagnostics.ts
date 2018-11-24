import { workspace, window, commands, OutputChannel, ConfigurationTarget, Uri } from "vscode";
import { inspect, format } from "util";
import * as ver from './ver';
import * as adapter from './adapter';
import * as util from './util';

enum DiagnosticsStatus {
    Succeeded = 0,
    Warning = 1,
    Failed = 2,
    NotFound = 3
}

export async function diagnose(output: OutputChannel): Promise<boolean> {
    output.clear();
    let status = DiagnosticsStatus.Succeeded;
    try {
        output.appendLine('--- Checking version ---');
        let versionPattern = '^lldb version ([0-9.]+)';
        let desiredVersion = '3.9.1';
        if (process.platform.includes('win32')) {
            desiredVersion = '4.0.0';
        } else if (process.platform.includes('darwin')) {
            versionPattern = '^lldb-([0-9.]+)';
            desiredVersion = '360.1.68';
        }
        let pattern = new RegExp(versionPattern, 'm');

        let config = workspace.getConfiguration('lldb', null);
        let adapterPathOrginal = config.get('executable', 'lldb');
        let adapterPath = adapterPathOrginal;
        let adapterEnv = config.get('executable_env', {});

        // Try to locate LLDB and get its version.
        let version: string = null;
        let lldbNames: string[];
        if (process.platform.includes('linux')) {
            // Linux tends to have versioned binaries only.
            lldbNames = ['lldb', 'lldb-10.0', 'lldb-9.0', 'lldb-8.0', 'lldb-7.0',
                'lldb-6.0', 'lldb-5.0', 'lldb-4.0', 'lldb-3.9'];
        } else {
            lldbNames = ['lldb'];
        }
        if (adapterPathOrginal != 'lldb') {
            lldbNames.unshift(adapterPathOrginal); // Also try the explicitly configured value.
        }
        for (let name of lldbNames) {
            try {
                let lldb = adapter.spawnDebugger(['-v'], name, adapterEnv);
                util.logProcessOutput(lldb, output);
                version = (await util.waitForPattern(lldb, lldb.stdout, pattern))[1];
                adapterPath = name;
                break;
            } catch (err) {
                output.appendLine(inspect(err));
            }
        }

        if (!version) {
            status = DiagnosticsStatus.NotFound;
        } else {
            if (ver.lt(version, desiredVersion)) {
                output.appendLine(
                    format('Warning: The version of your LLDB was detected as %s, which had never been tested with this extension. ' +
                        'Please consider upgrading to least version %s.',
                        version, desiredVersion));
                status = DiagnosticsStatus.Warning;
            }

            // Check if Python scripting is usable.
            output.appendLine('--- Checking Python ---');
            let lldb2 = adapter.spawnDebugger(['-b',
                '-O', 'script import sys, io, lldb',
                '-O', 'script print(lldb.SBDebugger.Create().IsValid())',
                '-O', 'script print("OK")'
            ], adapterPath, adapterEnv);
            util.logProcessOutput(lldb2, output);
            // [^] = match any char, including newline
            let match2 = await util.waitForPattern(lldb2, lldb2.stdout, new RegExp('^True$[^]*^OK$', 'm'));
        }
        output.appendLine('--- Done ---');
        output.show(true);

        // If we updated adapterPath, ask user what to do.
        if (adapterPathOrginal != adapterPath) {
            let action = await window.showInformationMessage(
                format('Could not launch LLDB executable "%s", ' +
                    'however we did locate a usable LLDB binary: "%s". ' +
                    'Would you like to update LLDB configuration with this value?',
                    adapterPathOrginal, adapterPath),
                'Yes', 'No');
            if (action == 'Yes') {
                output.appendLine('Setting "lldb.executable": "' + adapterPath + '".');
                config.update('executable', adapterPath, ConfigurationTarget.Global);
            } else {
                status = DiagnosticsStatus.Failed;
            }
        }
    } catch (err) {
        output.appendLine('');
        output.appendLine('*** An exception was raised during self-test ***');
        output.appendLine(inspect(err));
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
            window.showErrorMessage('LLDB self-test has failed!');
            break;
        case DiagnosticsStatus.NotFound:
            let action = await window.showErrorMessage('Could not find LLDB on your system.', 'Show installation instructions');
            if (action != null)
                commands.executeCommand('vscode.open', Uri.parse('https://github.com/vadimcn/vscode-lldb/wiki/Installing-LLDB'));
            break;
    }
    return status < DiagnosticsStatus.Failed;
}

export async function analyzeStartupError(err: Error, output: OutputChannel) {
    output.appendLine(err.toString());
    output.show(true)
    let e = <any>err;
    let diagnostics = 'Run diagnostics';
    let actionAsync;
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
        await diagnose(output);
    }
}
