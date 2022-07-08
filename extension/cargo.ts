import {
    tasks, workspace, DebugConfiguration, CustomExecution, EventEmitter, Pseudoterminal, Task, WorkspaceFolder,
    CancellationToken, ConfigurationScope, Uri
} from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as readline from 'readline';
import { inspect } from 'util';
import * as async from './novsc/async';
import { Dict, Environment } from './novsc/commonTypes';
import { output } from './main';
import { expandVariablesInObject, mergedEnvironment } from './novsc/expand';

export interface CargoConfig {
    type: string,
    args?: string[];
    env?: Dict<string>;
    problemMatcher?: string | string[];
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

export class Cargo {
    folder: WorkspaceFolder;
    cancellation?: CancellationToken

    public constructor(folder: WorkspaceFolder, cancellation?: CancellationToken) {
        this.folder = folder;
        this.cancellation = cancellation;
    }

    getCargoTomlDir(): string {
        return this.folder?.uri?.fsPath;
    }

    public async getProgramFromCargoConfig(
        cargoConfig: CargoConfig,
    ): Promise<string> {

        let artifacts: CompilationArtifact[] = [];

        let execution = new CustomExecution(async taskDef => {
            let outputEmitter = new EventEmitter<string>();
            let doneEmitter = new EventEmitter<number>();
            let pty: Pseudoterminal = {
                onDidWrite: outputEmitter.event,
                onDidClose: doneEmitter.event,
                open: async () => {
                    let cargoArgs = taskDef.args;
                    let pos = cargoArgs.indexOf('--');
                    // Insert either before `--` or at the end.
                    cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, '--message-format=json');

                    outputEmitter.fire('Running `cargo ' + cargoArgs.join(' ') + '`...\r\n');
                    let newline = /\n/g;
                    try {
                        artifacts = await this.getCargoArtifacts(
                            taskDef.args,
                            taskDef.env,
                            message => outputEmitter.fire(message.replace(newline, '\r\n'))
                        );
                        doneEmitter.fire(0);
                    } catch (error) {
                        output.appendLine(error);
                        doneEmitter.fire(1);
                    }
                },
                close: () => { }
            };
            return pty;
        });

        let problemMatchers = cargoConfig.problemMatcher || '$rustc';
        cargoConfig.type = 'cargo';
        let task = new Task(cargoConfig, this.folder, 'cargo', 'CodeLLDB', execution, problemMatchers);
        task.presentationOptions.clear = true;
        let taskExecution = await tasks.executeTask(task);

        // Wait for the task to end
        await new Promise<void>(resolve => {
            tasks.onDidEndTask(e => {
                if (e.execution == taskExecution) {
                    resolve();
                }
            });
        });

        return this.getProgramFromArtifacts(artifacts);
    }

    getProgramFromArtifacts(artifacts: CompilationArtifact[], filter?: { name?: string; kind?: string }): string {
        output.appendLine('Raw artifacts:');
        for (let artifact of artifacts) {
            output.appendLine(inspect(artifact));
        }

        if (filter != undefined) {
            artifacts = artifacts.filter(a => {
                if (filter.name != undefined && a.name != filter.name)
                    return false;
                if (filter.kind != undefined && a.kind != filter.kind)
                    return false;
                return true;
            });
        }

        output.appendLine('Filtered artifacts: ');
        for (let artifact of artifacts) {
            output.appendLine(inspect(artifact));
        }

        if (artifacts.length == 0) {
            throw new Error('Cargo has produced no matching compilation artifacts.');
        } else if (artifacts.length > 1) {
            throw new Error('Cargo has produced more than one matching compilation artifact.');
        }

        return artifacts[0].fileName;
    }

    // Runs cargo, returns a list of compilation artifacts.
    async getCargoArtifacts(
        cargoArgs: string[],
        cargoEnv: Environment,
        onMessage: (data: string) => void
    ): Promise<CompilationArtifact[]> {
        let artifacts: CompilationArtifact[] = [];
        try {
            let cargoCwd = this.getCargoTomlDir();
            await this.runCargo(cargoArgs, cargoEnv, cargoCwd,
                message => {
                    if (message.reason == 'compiler-artifact') {
                        let isBinary = message.target.crate_types.includes('bin');
                        let isBuildScript = message.target.kind.includes('custom-build');
                        if ((isBinary && !isBuildScript) || message.profile.test) {
                            if (message.executable !== undefined) {
                                if (message.executable !== null) {
                                    artifacts.push({
                                        fileName: message.executable,
                                        name: message.target.name,
                                        kind: message.target.kind[0]
                                    });
                                }
                            } else { // Older cargo
                                for (let i = 0; i < message.filenames.length; ++i) {
                                    if (message.filenames[i].endsWith('.dSYM'))
                                        continue;
                                    artifacts.push({
                                        fileName: message.filenames[i],
                                        name: message.target.name,
                                        kind: message.target.kind[i]
                                    });
                                }
                            }
                        }
                    } else if (message.reason == 'compiler-message') {
                        onMessage(message.message.rendered)
                    }
                },
                onMessage
            );
        } catch (err) {
            throw new Error(`Cargo invocation has failed: ${err}`);
        }
        return artifacts;
    }

    public async getLaunchConfigs(): Promise<DebugConfiguration[]> {

        let cargoTomlFolder = this.getCargoTomlDir();
        if (!await async.fs.exists(path.join(cargoTomlFolder, 'Cargo.toml')))
            return [];

        let metadata: any = null;

        await this.runCargo(
            ['metadata', '--no-deps', '--format-version=1'],
            new Environment(),
            cargoTomlFolder,
            m => { metadata = m },
            stderr => { output.append(stderr); },
        );
        if (!metadata)
            throw new Error(`Cargo has produced no metadata`);

        let configs: DebugConfiguration[] = [];
        for (let pkg of metadata.packages) {
            function addConfig(name: string, cargo_args: string[], filter: any) {
                configs.push({
                    type: 'lldb',
                    request: 'launch',
                    name: name,
                    cargo: {
                        args: cargo_args.concat(`--package=${pkg.name}`),
                        filter: filter
                    },
                    args: [],
                    cwd: '${workspaceFolder}'
                });
            };

            for (let target of pkg.targets) {
                let libAdded = false;
                for (let kind of target.kind) {
                    switch (kind) {
                        case 'lib':
                        case 'rlib':
                        case 'staticlib':
                        case 'dylib':
                        case 'cstaticlib':
                            if (!libAdded) {
                                addConfig(`Debug unit tests in library '${target.name}'`,
                                    ['test', '--no-run', '--lib'],
                                    { name: target.name, kind: 'lib' });
                                libAdded = true;
                            }
                            break;

                        case 'bin':
                        case 'example':
                            {
                                let prettyKind = (kind == 'bin') ? 'executable' : kind;
                                addConfig(`Debug ${prettyKind} '${target.name}'`,
                                    ['build', `--${kind}=${target.name}`],
                                    { name: target.name, kind: kind });
                                addConfig(`Debug unit tests in ${prettyKind} '${target.name}'`,
                                    ['test', '--no-run', `--${kind}=${target.name}`],
                                    { name: target.name, kind: kind });
                            }
                            break;

                        case 'bench':
                        case 'test':
                            {
                                let prettyKind = (kind == 'bench') ? 'benchmark' : (kind == 'test') ? 'integration test' : kind;
                                addConfig(`Debug ${prettyKind} '${target.name}'`,
                                    ['test', '--no-run', `--${kind}=${target.name}`],
                                    { name: target.name, kind: kind });
                            }
                            break;
                    }
                }
            }
        }
        return configs;
    }

    // Runs cargo, invokes stdout/stderr callbacks as data comes in, returns the exit code.
    async runCargo(
        args: string[],
        env: Environment,
        cwd: string,
        onStdoutJson: (obj: any) => void,
        onStderrString: (data: string) => void,
    ): Promise<number> {
        let config = workspace.getConfiguration('lldb', this.folder);
        let cargoCmd = config.get<string>('cargo');

        return new Promise<number>((resolve, reject) => {
            let cargo = cp.spawn(cargoCmd, args, {
                stdio: ['ignore', 'pipe', 'pipe'],
                cwd: cwd,
                env: mergedEnvironment(env),
            });

            cargo.on('error', err => {
                reject(new Error(`could not launch cargo: ${err}`));
            });

            cargo.stderr.on('data', chunk => {
                onStderrString(chunk.toString());
            });

            let rl = readline.createInterface({ input: cargo.stdout });
            rl.on('line', line => {
                const message = JSON.parse(line);
                onStdoutJson(message);
            });

            cargo.on('exit', (exitCode, signal) => {
                if (exitCode == 0)
                    resolve(0);
                else
                    reject(new Error(`exit code: ${exitCode}.`));
            });

            if (this.cancellation) {
                this.cancellation.onCancellationRequested(e => cargo.kill('SIGINT'));
            }
        });
    }
}

// Expands ${cargo:...} placeholders.
export function expandCargo(launchConfig: DebugConfiguration, cargoDict: Dict<string>): DebugConfiguration {
    let expander = (type: string, key: string) => {
        if (type == 'cargo') {
            let value = cargoDict[key];
            if (value == undefined)
                throw new Error('cargo:' + key + ' is not defined');
            return value.toString();
        }
    };
    return expandVariablesInObject(launchConfig, expander);
}
