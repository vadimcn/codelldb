import { workspace, window, QuickPickItem, DebugConfiguration } from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { output } from './extension';

export async function getProgramFromCargo(cargoConfig: any): Promise<string> {
    let cargoArgs = cargoConfig.args;
    let pos = cargoArgs.indexOf('--');
    // Insert either before `--` or at the end.
    cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, '--message-format=json');
    output.appendLine('Running `cargo ' + cargoArgs.join(' ') + '`...');
    let artifacts = await getCargoArtifacts(cargoArgs);
    output.appendLine('Cargo artifacts: ' + artifacts.join(', '));
    if (artifacts.length < 1) {
        output.show();
        throw new Error('Cargo produced no binary artifacts.')
    }
    if (artifacts.length > 1) {
        output.show();
        window.showWarningMessage('Cargo produced more than one binary artifact.')
    }
    return artifacts[0];
}

// Runs cargo, returns a list of compilation artifacts.
async function getCargoArtifacts(cargoArgs: string[]): Promise<string[]> {
    var artifacts: string[] = [];
    let exitCode = await runCargo(cargoArgs,
        message => {
            if (message.reason == 'compiler-artifact') {
                if (message.target.crate_types.indexOf('bin') >= 0 ||
                    message.profile.test) {
                    artifacts = artifacts.concat(message.filenames);
                }
            } else if (message.reason == 'compiler-message') {
                output.appendLine(message.message.rendered);
            }
        },
        stderr => { output.append(stderr); }
    );
    if (exitCode != 0) {
        output.show();
        throw new Error('Cargo invocation has failed (exit code: ' + exitCode.toString() + ').');
    }
    return artifacts;
}


export async function getLaunchConfigs(): Promise<DebugConfiguration[]> {
    let configs = [];
    if (fs.existsSync(path.join(workspace.rootPath, 'Cargo.toml'))) {
        var metadata: any = null;
        let exitCode = await runCargo(['metadata', '--no-deps', '--format-version=1'],
            m => { metadata = m },
            stderr => { output.append(stderr); }
        );

        if (metadata && exitCode == 0) {
            for (var pkg of metadata.packages) {
                for (var target of pkg.targets) {
                    let target_kinds = target.kind as string[];

                    var debug_selector = null;
                    var test_selector = null;
                    if (target_kinds.indexOf('bin') >= 0) {
                        debug_selector = ['--bin=' + target.name];
                        test_selector = ['--bin=' + target.name];
                    }
                    if (target_kinds.indexOf('test') >= 0) {
                        debug_selector = ['--test=' + target.name];
                        test_selector = ['--test=' + target.name];
                    }
                    if (target_kinds.indexOf('lib') >= 0) {
                        test_selector = ['--lib'];
                    }

                    if (debug_selector) {
                        configs.push({
                            type: 'lldb',
                            request: 'launch',
                            name: 'Debug ' + target.name,
                            cargo: { args: ['build'].concat(debug_selector) },
                            args: [],
                            cwd: '${workspaceFolder}'
                        });
                    }
                    if (test_selector) {
                        configs.push({
                            type: 'lldb',
                            request: 'launch',
                            name: 'Debug tests in ' + target.name,
                            cargo: { args: ['test', '--no-run'].concat(test_selector) },
                            args: [],
                            cwd: '${workspaceFolder}'
                        });
                    }
                }
            }
        }
    }
    return configs;
}

// Runs cargo, invokes stdout/stderr callbacks as data comes in, returns the exit code.
async function runCargo(
    cargoArgs: string[],
    onStdoutJson: (obj: any) => void,
    onStderrString: (data: string) => void
): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        let cargo = cp.spawn('cargo', cargoArgs, {
            stdio: ['ignore', 'pipe', 'pipe'],
            cwd: workspace.rootPath
        });

        cargo.on('error', err => reject(err));

        cargo.stderr.on('data', chunk => {
            onStderrString(chunk.toString());
        });

        var stdout = '';
        cargo.stdout.on('data', chunk => {
            stdout += chunk
            let lines = stdout.split('\n');
            stdout = lines.pop();
            for (var line of lines) {
                let message = JSON.parse(line);
                onStdoutJson(message);
            }
        });

        cargo.on('exit', (exitCode, signal) => {
            resolve(exitCode);
        });
    });
}
