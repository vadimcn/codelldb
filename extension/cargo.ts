import { workspace, window, QuickPickItem, DebugConfiguration, WorkspaceFolder } from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { inspect } from 'util';
import * as util from './util';
import { output, Dict } from './extension';

export interface CargoConfig {
    args: string[];
    filter?: {
        name?: string;
        kind?: string;
    }
}

interface CompilationArtifact {
    fileName: string;
    name: string;
    kind: string
}

export async function getProgramFromCargo(cargoConfig: CargoConfig, cwd: string): Promise<string> {
    let cargoArgs = cargoConfig.args;
    let pos = cargoArgs.indexOf('--');
    // Insert either before `--` or at the end.
    cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, '--message-format=json');

    output.appendLine('Running `cargo ' + cargoArgs.join(' ') + '`...');
    let artifacts = await getCargoArtifacts(cargoArgs, cwd);

    if (artifacts.length == 0) {
        output.show();
        window.showErrorMessage('Cargo has produced no binary artifacts.', { modal: true });
        throw new Error('Cannot start debugging.');
    }

    if (cargoConfig.filter != undefined) {
        let filter = cargoConfig.filter;
        artifacts = artifacts.filter(a => {
            if (filter.name != undefined && a.name != filter.name)
                return false;
            if (filter.kind != undefined && a.kind != filter.kind)
                return false;
            return true;
        });
    }

    output.appendLine('Matching compilation artifacts: ');
    for (var artifact of artifacts) {
        output.appendLine(inspect(artifact));
    }
    if (artifacts.length > 1) {
        output.show();
        window.showErrorMessage('Cargo has produced more than one binary artifact.', { modal: true });
        throw new Error('Cannot start debugging.');
    }
    return artifacts[0].fileName;
}

// Runs cargo, returns a list of compilation artifacts.
async function getCargoArtifacts(cargoArgs: string[], folder: string): Promise<CompilationArtifact[]> {
    var artifacts: CompilationArtifact[] = [];
    let exitCode = await runCargo(cargoArgs, folder,
        message => {
            if (message.reason == 'compiler-artifact') {
                let isBinary = message.target.crate_types.includes('bin');
                let isBuildScript = message.target.kind.includes('custom-build');
                if ((isBinary && !isBuildScript) || message.profile.test) {
                    for (var i = 0; i < message.filenames.length; ++i) {
                        if (message.filenames[i].endsWith('.dSYM'))
                            continue;
                        artifacts.push({
                            fileName: message.filenames[i],
                            name: message.target.name,
                            kind: message.target.kind[i]
                        });
                    }
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


export async function getLaunchConfigs(folder: string): Promise<DebugConfiguration[]> {
    let configs = [];
    if (fs.existsSync(path.join(folder, 'Cargo.toml'))) {
        var metadata: any = null;
        let exitCode = await runCargo(['metadata', '--no-deps', '--format-version=1'], folder,
            m => { metadata = m },
            stderr => { output.append(stderr); }
        );

        if (metadata && exitCode == 0) {
            for (var pkg of metadata.packages) {
                for (var target of pkg.targets) {
                    let target_kinds = target.kind as string[];

                    var debug_selector = null;
                    var test_selector = null;
                    if (target_kinds.includes('bin')) {
                        debug_selector = ['--bin=' + target.name];
                        test_selector = ['--bin=' + target.name];
                    }
                    if (target_kinds.includes('test')) {
                        debug_selector = ['--test=' + target.name];
                        test_selector = ['--test=' + target.name];
                    }
                    if (target_kinds.includes('lib')) {
                        test_selector = ['--lib'];
                    }

                    if (debug_selector) {
                        configs.push({
                            type: 'lldb',
                            request: 'launch',
                            name: 'Debug ' + target.name,
                            cargo: { args: ['build', `--package=${pkg.name}`].concat(debug_selector) },
                            args: [],
                            cwd: '${workspaceFolder}'
                        });
                    }
                    if (test_selector) {
                        configs.push({
                            type: 'lldb',
                            request: 'launch',
                            name: 'Debug tests in ' + target.name,
                            cargo: { args: ['test', '--no-run', `--package=${pkg.name}`].concat(test_selector) },
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
    cargoArgs: string[], cwd: string,
    onStdoutJson: (obj: any) => void,
    onStderrString: (data: string) => void
): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        let cargo = cp.spawn('cargo', cargoArgs, {
            stdio: ['ignore', 'pipe', 'pipe'],
            cwd: cwd
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

export function expandCargo(launchConfig: DebugConfiguration, cargoDict: Dict<string>): DebugConfiguration {
    let expander = (type: string, key: string) => {
        if (type == 'cargo') {
            let value = cargoDict[key];
            if (value == undefined)
                throw new Error('cargo:' + key + ' is not defined');
            return value.toString();
        }
    };
    return util.expandVariablesInObject(launchConfig, expander);
}
