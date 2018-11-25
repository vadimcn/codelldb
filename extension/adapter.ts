import {
    workspace, languages, window, commands,
    ExtensionContext, Disposable, QuickPickItem, Uri, Event, EventEmitter, OutputChannel, ConfigurationTarget,
    WorkspaceFolder, WorkspaceConfiguration
} from 'vscode';
import { format, inspect } from 'util';
import * as cp from 'child_process';
import * as path from 'path';
import * as util from './util';
import { Dict } from './util';
import { output } from './main';

export class AdapterProcess {
    public isAlive: boolean;
    public port: number;

    constructor(process: cp.ChildProcess) {
        this.process = process;
        this.isAlive = true;
        process.on('exit', (code, signal) => {
            this.isAlive = false;
            if (signal) {
                output.appendLine(format('Adapter terminated by %s signal.', signal));
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
export async function startDebugAdapter(
    context: ExtensionContext,
    folder: WorkspaceFolder | undefined,
    params: Dict<any>
): Promise<AdapterProcess> {
    let config = workspace.getConfiguration('lldb', folder ? folder.uri : undefined);
    let adapterArgs: string[];
    let adapterExe: string;
    let adapterEnv = config.get('executable_env', {});
    let paramsBase64 = getAdapterParameters(config, params);
    adapterArgs = ['-b',
        '-O', format('command script import \'%s\'', path.join(context.extensionPath, 'adapter')),
        '-O', format('script adapter.run_tcp_session(0, \'%s\')', paramsBase64)
    ];
    adapterExe = config.get('executable', 'lldb');
    let adapter = spawnDebugger(adapterArgs, adapterExe, adapterEnv);
    let regex = new RegExp('^Listening on port (\\d+)\\s', 'm');
    util.logProcessOutput(adapter, output);
    let match = await util.waitForPattern(adapter, adapter.stdout, regex);

    let adapterProc = new AdapterProcess(adapter);
    adapterProc.port = parseInt(match[1]);
    return adapterProc;
}

function setIfDefined(target: Dict<any>, config: WorkspaceConfiguration, key: string) {
    let value = util.getConfigNoDefault(config, key);
    if (value !== undefined)
        target[key] = value;
}

function getAdapterParameters(config: WorkspaceConfiguration, params: Dict<any>): string {
    setIfDefined(params, config, 'logLevel');
    setIfDefined(params, config, 'loggers');
    setIfDefined(params, config, 'logFile');
    setIfDefined(params, config, 'reverseDebugging');
    setIfDefined(params, config, 'suppressMissingSourceFiles');
    setIfDefined(params, config, 'evaluationTimeout');
    setIfDefined(params, config, 'ptvsd');
    return new Buffer(JSON.stringify(params)).toString('base64');
}


// Spawn LLDB with the specified arguments, wait for it to output something matching
// regex pattern, or until the timeout expires.
export function spawnDebugger(args: string[], adapterPath: string, adapterEnv: Dict<string>): cp.ChildProcess {
    let env = Object.assign({}, process.env);
    for (let key in adapterEnv) {
        env[key] = util.expandVariables(adapterEnv[key], (type, key) => {
            if (type == 'env') return process.env[key];
            throw new Error('Unknown variable type ' + type);
        });
    }

    let options: cp.SpawnOptions = {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: env,
        cwd: workspace.rootPath
    };
    if (process.platform.includes('darwin')) {
        // Make sure LLDB finds system Python before Brew Python
        // https://github.com/Homebrew/legacy-homebrew/issues/47201
        options.env['PATH'] = '/usr/bin:' + process.env['PATH'];
    }
    return cp.spawn(adapterPath, args, options);
}
