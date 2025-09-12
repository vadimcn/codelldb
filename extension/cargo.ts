import {
    tasks, DebugConfiguration, CustomExecution, EventEmitter, Pseudoterminal, Task, WorkspaceFolder, CancellationToken,
    TaskDefinition, TaskScope, ExtensionContext
} from 'vscode';
import * as cp from 'child_process';
import * as readline from 'readline';
import * as net from 'net';
import * as path from 'path';
import { inspect } from 'util';
import { Dict } from './novsc/commonTypes';
import { getExtensionConfig } from './main';
import { output } from './logging';
import { expandVariablesInObject } from './novsc/expand';
import { LaunchEnvironment } from 'codelldb';
import { RpcServer, waitEndOfDebugSession } from './externalLaunch';
import YAML from 'yaml';

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
    context: ExtensionContext;
    workspaceFolder?: WorkspaceFolder;
    cancellation?: CancellationToken;

    public constructor(context: ExtensionContext, workspaceFolder?: WorkspaceFolder, cancellation?: CancellationToken) {
        this.context = context;
        this.workspaceFolder = workspaceFolder;
        this.cancellation = cancellation;
    }

    public async resolveCargoConfig(debugConfig: DebugConfiguration): Promise<DebugConfiguration> {

        let cargoConfig = debugConfig.cargo;
        // Handle "cargo": [...] form
        if (cargoConfig instanceof Array) {
            cargoConfig = { args: cargoConfig }
        }

        let rpcResolve: (value: LaunchEnvironment) => void;
        let rpcRequestPromise = new Promise<LaunchEnvironment>(resolve => rpcResolve = resolve);
        let rpcRespond: ((success: boolean) => void) | undefined;
        let rpcServer = new RpcServer(request => {
            let launchEnv: LaunchEnvironment = YAML.parse(request);
            // RPC response is delayed until the end of the debug session to keep the launcher active.
            let responsePromise = new Promise<string>(resolve => {
                rpcRespond = (success: boolean) => resolve(`{ "success": ${success} }`);
            });
            rpcResolve(launchEnv);
            return responsePromise;
        });
        let address = await rpcServer.listen({ host: '127.0.0.1', port: 0 }) as net.AddressInfo;

        try {
            let runner = path.join(this.context.extensionPath, 'adapter', 'codelldb-launch');
            let extraArgs = ['--message-format=json', '--color=always',
                `--config=target.'cfg(all())'.runner='${runner}'`
            ];
            let cargoArgs = cargoConfig.args || [];
            // Insert extraArgs either before `--` or at the end.
            let pos = cargoArgs.indexOf('--');
            cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, ...extraArgs);
            let cargoEnv = Object.assign({}, cargoConfig.env);
            cargoEnv['CODELLDB_LAUNCH_CONNECT'] = `${address.address}:${address.port}`;

            let task = new Task(
                { type: undefined, command: '' } as unknown as TaskDefinition,
                this.workspaceFolder ?? TaskScope.Workspace,
                'cargo', 'CodeLLDB', undefined, cargoConfig.problemMatcher ?? '$codelldb-rustc');
            task.presentationOptions = { clear: true, showReuseMessage: false };

            let artifactsPromise = runTask(task, async (_, write) => {
                let artifacts = await this.runCargoAndGetArtifacts(cargoArgs, cargoEnv, cargoConfig.cwd, write);
                if (rpcRespond) // This means that rpcPromise is already resolved
                    return '';
                return this.getProgramFromArtifacts(artifacts, cargoConfig.filter);
            });

            let result = await Promise.race([rpcRequestPromise, artifactsPromise]);
            if (typeof result == 'object') {
                // Case 1: `cargo run ...` is used, and our injected runner connects to the RPC endpoint
                // and sends LaunchEnvironment info including the debuggee path, arguments, etc.
                let launchEnv = result as LaunchEnvironment;
                // Use args passed in by Cargo, appending any user-provided args
                debugConfig.program = launchEnv.cmd[0];
                debugConfig.args = launchEnv.cmd.slice(1).concat(debugConfig.args ?? []);
                debugConfig.cwd = launchEnv.cwd;
                // Use Cargo environment, with overrides from launchConfig
                debugConfig.env = Object.assign({}, debugConfig.env, launchEnv.env);
                debugConfig = expandCargo(debugConfig, { program: launchEnv.cmd[0] });
            } else {
                // Case 2: `cargo build ...` is used; the `result` is the path of the debuggee executable.
                debugConfig = expandCargo(debugConfig, { program: result }); // Expand ${cargo:program}.
                if (debugConfig.program == undefined) {
                    debugConfig.program = result;
                }
            }
            // If launch was initiated via RPC (case 1), we need to dismiss the launcher at the end of the session.
            if (rpcRespond) {
                waitEndOfDebugSession(debugConfig).then(success => {
                    rpcRespond!(success);
                    rpcServer.close();
                });
            }
        } finally {
            if (!rpcRespond)
                rpcServer.close();
        }

        // Add 'rust' to sourceLanguages, since this project obviously involves Rust.
        debugConfig.sourceLanguages = debugConfig.sourceLanguages || [];
        debugConfig.sourceLanguages.push('rust');

        delete debugConfig.cargo;
        return debugConfig;
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
    async runCargoAndGetArtifacts(
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
        let metadata: any = undefined;
        await this.runCargo(['metadata', '--no-deps', '--format-version=1'], {}, directory,
            m => { metadata = m },
            stderr => { output.append(stderr); },
        );
        return metadata ? this.debugConfigsFromCargoMetadata(metadata, directory) : [];
    }

    debugConfigsFromCargoMetadata(metadata: any, directory?: string): DebugConfiguration[] {
        let config = getExtensionConfig(this.workspaceFolder);
        if (config.get<boolean>('generateOldCargoConfig', false))
            return this.debugConfigsFromCargoMetadataOld(metadata, directory);

        let configs: DebugConfiguration[] = [];
        for (let pkg of metadata.packages) {

            function addConfig(name: string, cargoArgs: string[]) {
                if (metadata.packages.length > 1)
                    cargoArgs.concat(`--package=${pkg.name}`);

                let config: DebugConfiguration = {
                    type: 'lldb',
                    request: 'launch',
                    name: name,
                    cargo: {
                        args: cargoArgs,
                    },
                };
                if (cargoArgs[0] == 'run')
                    config.args = [];
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
                                    ['test', '--lib']);
                                libAdded = true;
                            }
                            break;

                        case 'bin':
                        case 'example':
                            {
                                let prettyKind = (kind == 'bin') ? 'executable' : kind;
                                addConfig(`Debug ${prettyKind} '${target.name}'`,
                                    ['run', `--${kind}=${target.name}`]);
                                addConfig(`Debug unit tests in ${prettyKind} '${target.name}'`,
                                    ['test', `--${kind}=${target.name}`]);
                            }
                            break;

                        case 'bench':
                        case 'test':
                            {
                                let prettyKind = (kind == 'bench') ? 'benchmark' : (kind == 'test') ? 'integration test' : kind;
                                addConfig(`Debug ${prettyKind} '${target.name}'`,
                                    ['test', `--${kind}=${target.name}`]);
                            }
                            break;
                    }
                }
            }
        }
        return configs;
    }

    debugConfigsFromCargoMetadataOld(metadata: any, directory?: string): DebugConfiguration[] {
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

        output.appendLine(`Running: ${cargoCmd} ${args.join(' ')}`);
        return new Promise<number>((resolve, reject) => {
            let cargo = cp.spawn(cargoCmd, args, {
                stdio: ['ignore', 'pipe', 'pipe'],
                cwd: cargoCwd,
                env: cargoEnv,
            });

            cargo.on('error', err => {
                if ((err as any).code == 'ENOENT')
                    err.message = `Cargo: could not find "${cargoCmd}" executable.`;
                output.appendLine(err.message);
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

            cargo.on('exit', (code, signal) => {
                output.appendLine(`Cargo exited with code ${code} ${signal ? 'signal=' + signal : ''}`);
                resolve(code ?? -1);
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

// Expands ${cargo: ...} placeholders.
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
