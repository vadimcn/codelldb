import { window, DebugConfiguration, env } from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as readline from 'readline';
import { inspect } from 'util';
import * as async from './novsc/async';
import { Dict } from './novsc/commonTypes';
import { output } from './main';
import { expandVariablesInObject, mergedEnvironment } from './novsc/expand';

export interface CargoConfig {
    args: string[];
    filter?: {
        name?: string;
        kind?: string;
    },
    overrideCargo?: string
}

interface CompilationArtifact {
    fileName: string;
    name: string;
    kind: string
}

export class Cargo {
    cargoTomlFolder: string;
    extraEnv: Dict<string>;

    public constructor(cargoTomlFolder: string, extraEnv: Dict<string>) {
        this.cargoTomlFolder = cargoTomlFolder;
        this.extraEnv = extraEnv;
    }

    public async getProgramFromCargoConfig(cargoConfig: CargoConfig): Promise<string> {
        let cargoArgs = cargoConfig.args;
        let cargoProg = cargoConfig.overrideCargo || "cargo";
        let pos = cargoArgs.indexOf('--');
        // Insert either before `--` or at the end.
        cargoArgs.splice(pos >= 0 ? pos : cargoArgs.length, 0, '--message-format=json');

        output.appendLine(`Running \`${cargoProg} ${cargoArgs.join(' ')}\`...`);
        let artifacts = await this.getCargoArtifacts(cargoProg, cargoArgs);

        output.appendLine('Raw artifacts:');
        for (let artifact of artifacts) {
            output.appendLine(inspect(artifact));
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

            output.appendLine('Filtered artifacts: ');
            for (let artifact of artifacts) {
                output.appendLine(inspect(artifact));
            }
        }

        if (artifacts.length == 0) {
            output.show();
            window.showErrorMessage('Cargo has produced no matching compilation artifacts.', { modal: true });
            throw new Error('Cannot start debugging.');
        } else if (artifacts.length > 1) {
            output.show();
            window.showErrorMessage('Cargo has produced more than one matching compilation artifact.', { modal: true });
            throw new Error('Cannot start debugging.');
        }

        return artifacts[0].fileName;
    }

    // Runs cargo, returns a list of compilation artifacts.
    async  getCargoArtifacts(cargoProg: string, cargoArgs: string[]): Promise<CompilationArtifact[]> {
        let artifacts: CompilationArtifact[] = [];
        try {
            await this.runCargo(cargoProg, cargoArgs,
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
                        output.appendLine(message.message.rendered);
                    }
                },
                stderr => { output.append(stderr); }
            );
        } catch (err) {
            output.show();
            throw new Error(`Cargo invocation has failed: ${err}`);
        }
        return artifacts;
    }

    public async getLaunchConfigs(): Promise<DebugConfiguration[]> {
        if (!await async.fs.exists(path.join(this.cargoTomlFolder, 'Cargo.toml')))
            return [];

        let metadata: any = null;

        await this.runCargo('cargo', ['metadata', '--no-deps', '--format-version=1'],
            m => { metadata = m },
            stderr => { output.append(stderr); }
        );
        if (!metadata)
            throw new Error(`Cargo produced no metadata`);

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
    runCargo(
        cargoProg: string,
        cargoArgs: string[],
        onStdoutJson: (obj: any) => void,
        onStderrString: (data: string) => void
    ): Promise<number> {
        return new Promise<number>((resolve, reject) => {
            const fullCommand = [...cargoProg.split(" "), ...cargoArgs];
            let cargo = cp.spawn(fullCommand[0], fullCommand.slice(1), {
                stdio: ['ignore', 'pipe', 'pipe'],
                cwd: this.cargoTomlFolder,
                env: mergedEnvironment(this.extraEnv),
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
                    resolve(exitCode);
                else
                    reject(new Error(`exit code: ${exitCode}.`));
            });
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
