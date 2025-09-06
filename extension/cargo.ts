import {
    tasks, DebugConfiguration, CustomExecution, EventEmitter, Pseudoterminal, Task, WorkspaceFolder, CancellationToken,
    TaskDefinition, TaskScope,
} from 'vscode';
import * as cp from 'child_process';
import * as readline from 'readline';
import { inspect } from 'util';
import { Dict } from './novsc/commonTypes';
import { output, getExtensionConfig } from './main';
import { expandVariablesInObject } from './novsc/expand';

export interface CargoConfig {
    args?: string[];
    cwd?: string,
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
    workspaceFolder?: WorkspaceFolder;
    cancellation?: CancellationToken;

    public constructor(workspaceFolder?: WorkspaceFolder, cancellation?: CancellationToken) {
        this.workspaceFolder = workspaceFolder;
        this.cancellation = cancellation;
    }

    public async getProgramFromCargoConfig(
        cargoConfig: CargoConfig,
        launcher?: { executable: string, env: Dict<string> }
    ): Promise<string> {

        let taskDef = Object.assign({ type: undefined, command: '' }, cargoConfig) as unknown as TaskDefinition;
        let taskScope = this.workspaceFolder || TaskScope.Workspace;
        let task = new Task(taskDef, taskScope, 'cargo', 'CodeLLDB', undefined, cargoConfig.problemMatcher);
        task.presentationOptions.clear = true;
        task.presentationOptions.showReuseMessage = false;
        let artifacts = await runTask(task, async (cargoConfig: CargoConfig, write) => {
            let cargoArgs = cargoConfig.args || [];
            // Insert either before `--` or at the end.
            let extraArgs = ['--message-format=json', '--color=always'];
            if (launcher) {
                extraArgs.push(`--config=target.'cfg(all())'.runner='${launcher.executable}'`);
            }
            let pos = cargoArgs.indexOf('--');
            cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, ...extraArgs);

            let cargoEnv = Object.assign({}, launcher?.env, cargoConfig.env);
            return this.getCargoArtifacts(cargoConfig.args ?? [], cargoEnv, cargoConfig.cwd, write);
        });
        return this.getProgramFromArtifacts(artifacts, cargoConfig.filter);
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
        cargoEnv: Dict<string>,
        cargoCwd: string | undefined,
        onMessage: (data: string) => void
    ): Promise<CompilationArtifact[]> {
        let artifacts: CompilationArtifact[] = [];
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
        return artifacts;
    }

    public async getLaunchConfigs(directory?: string): Promise<DebugConfiguration[]> {

        let metadata: any = null;

        let exitCode = await this.runCargo(
            ['metadata', '--no-deps', '--format-version=1'],
            {},
            directory,
            m => { metadata = m },
            stderr => { output.append(stderr); },
        );
        if (exitCode != 0)
            return []; // Most likely did not find Cargo.toml

        if (!metadata)
            throw new Error('Cargo has produced no metadata');

        let configs: DebugConfiguration[] = [];
        for (let pkg of metadata.packages) {
            function addConfig(name: string, cargo_args: string[], filter: any) {
                let config: DebugConfiguration = {
                    type: 'lldb',
                    request: 'launch',
                    name: name,
                    cargo: {
                        args: cargo_args.concat(`--package=${pkg.name}`),
                        filter: filter
                    },
                    args: [],
                    cwd: '${workspaceFolder}'
                };
                if (directory)
                    config.cargo.cwd = directory;
                configs.push(config);
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
        extraEnv: Dict<string>,
        cwd: string | undefined,
        onStdoutJson: (obj: any) => void,
        onStderrString: (data: string) => void,
    ): Promise<number> {
        let config = getExtensionConfig(this.workspaceFolder);
        let cargoCmd = config.get<string>('cargo', 'cargo');
        let cargoCwd = cwd ?? (this.workspaceFolder?.uri?.fsPath);
        let cargoEnv = Object.assign({}, process.env, extraEnv)

        output.appendLine(`Running ${cargoCmd} ${args.join(' ')}`);
        return new Promise<number>((resolve, reject) => {
            let cargo = cp.spawn(cargoCmd, args, {
                stdio: ['ignore', 'pipe', 'pipe'],
                cwd: cargoCwd,
                env: cargoEnv,
            });

            cargo.on('error', err => {
                if ((err as any).code == 'ENOENT')
                    err.message = `Could not find "${cargoCmd}" executable.`;
                reject(err);
            });

            cargo.stderr.on('data', chunk => {
                onStderrString(chunk.toString());
            });

            let rl = readline.createInterface({ input: cargo.stdout });
            rl.on('line', line => {
                if (line.startsWith('{')) {
                    let json;
                    try {
                        json = JSON.parse(line)
                    } catch (err) {
                        console.error(`Could not parse JSON: ${err} in "${line}"`);
                        return;
                    }
                    onStdoutJson(json);
                }
            });

            cargo.on('close', (exitCode) => {
                resolve(exitCode ?? -1);
            });

            if (this.cancellation) {
                this.cancellation.onCancellationRequested(e => cargo.kill('SIGINT'));
            }
        });
    }
}

async function runTask<T, R>(
    task: Task,
    execution: (resolvedTaskDef: T, write: (message: string) => void) => R | Promise<R>,
): Promise<R> {
    let result: any;
    let outputEmitter = new EventEmitter<string>();
    let doneEmitter = new EventEmitter<number>();
    task.execution = new CustomExecution(async resolvedTaskDef => {
        let pty: Pseudoterminal = {
            onDidWrite: outputEmitter.event,
            onDidClose: doneEmitter.event,
            open: async () => {
                try {
                    result = execution(resolvedTaskDef as T, message => {
                        outputEmitter.fire(message.replace(/\n/g, '\r\n'))
                    });
                    if (result instanceof Promise) {
                        result = await result;
                    }
                    doneEmitter.fire(0);
                } catch (err) {
                    result = err;
                    doneEmitter.fire(1);
                }
            },
            close: () => { }
        };
        return pty;
    });

    let exitCodePromise = new Promise<number>(resolve => doneEmitter.event(resolve));
    await tasks.executeTask(task);

    if (await exitCodePromise == 0) {
        return result;
    } else {
        throw result;
    }
}

// Expands ${cargo:...} placeholders.
export function expandCargo(launchConfig: DebugConfiguration, cargoDict: Dict<string>): DebugConfiguration {
    let expander = (type: string | null, key: string) => {
        if (type == 'cargo') {
            let value = cargoDict[key];
            if (value == undefined)
                throw new Error('cargo:' + key + ' is not defined');
            return value.toString();
        }
        return null;
    };
    return expandVariablesInObject(launchConfig, expander);
}
