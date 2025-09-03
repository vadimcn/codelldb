import * as cp from 'child_process';
import * as path from 'path';
import * as async from './async';
import * as os from 'os';
import { Dict  } from './commonTypes';
import { AdapterSettings } from 'codelldb';

export interface AdapterStartOptions {
    extensionRoot: string;
    liblldb?: string;
    workDir?: string;
    extraEnv: Dict<string>; // Extra environment to be set for adapter
    port: number;
    connect: boolean;  // Whether to connect or to listen on the port
    authToken?: string; // Token to use for authentication when reverse-connecting
    adapterSettings: AdapterSettings;
    verboseLogging: boolean;
}

export interface ProcessSpawnParams {
    command: string,
    args: string[],
    options: cp.SpawnOptions
}

export async function getSpawnParams(
    options: AdapterStartOptions
): Promise<ProcessSpawnParams> {
    let executable = path.join(options.extensionRoot, 'adapter', 'codelldb');

    let args: string[] = [];
    if (options.liblldb) {
        args.push('--liblldb', options.liblldb);
    }
    args.push(options.connect ? '--connect' : '--port', options.port.toString());
    if (options.authToken) {
        args.push('--auth-token', options.authToken);
    }
    if (options.adapterSettings) {
        args.push('--settings', JSON.stringify(options.adapterSettings));
    }

    let env = getAdapterEnv(options.extraEnv);
    env['RUST_TRACEBACK'] = '1';
    if (options.verboseLogging) {
        env['RUST_LOG'] = 'error,codelldb=debug';
    }

    // Check if workDir exists and is a directory, otherwise launch with default cwd.
    let workDir = options.workDir;
    if (workDir) {
        let stat = await async.fs.stat(workDir).catch(_ => null);
        if (!stat || !stat.isDirectory())
            workDir = undefined;
    }

    // Make sure that adapter gets launched with the correct architecture preference setting if
    // launched by translated x86 VSCode.
    if (await isRosetta()) {
        args = ['--arm64', executable].concat(args);
        executable = 'arch';
    }

    return {
        command: executable,
        args: args,
        options: {
            env: env,
            cwd: workDir
        }
    }
}

export async function start(options: AdapterStartOptions): Promise<cp.ChildProcess> {
    let spawnParams = await getSpawnParams(options);
    spawnParams.options.stdio = ['ignore', 'pipe', 'pipe'];
    return cp.spawn(spawnParams.command, spawnParams.args, spawnParams.options);
}

export async function findLibLLDB(pathHint: string): Promise<string | undefined> {
    let stat = await async.fs.stat(pathHint);
    if (stat.isFile())
        return pathHint;

    let libDir;
    let pattern;
    if (process.platform == 'linux') {
        libDir = path.join(pathHint, 'lib');
        pattern = /liblldb.*\.so.*/;
    } else if (process.platform == 'darwin') {
        libDir = path.join(pathHint, 'lib');
        pattern = /liblldb\..*dylib|LLDB/;
    } else if (process.platform == 'win32') {
        libDir = path.join(pathHint, 'bin');
        pattern = /liblldb\.dll/;
    } else {
        throw new Error('Unreachable');
    }

    for (let dir of [pathHint, libDir]) {
        let file = await findFileByPattern(dir, pattern);
        if (file) {
            return path.join(dir, file);
        }
    }
    return undefined;
}

async function findFileByPattern(path: string, pattern: RegExp): Promise<string | undefined> {
    try {
        let files = await async.fs.readdir(path);
        for (let file of files) {
            if (pattern.test(file))
                return file;
        }
    }
    catch (err) {
        // Ignore missing diractories and such...
    }
    return undefined;
}

export function getAdapterEnv(extraEnv: Dict<string>): Dict<string> {
    let env = Object.assign({}, process.env, extraEnv);
    // Scrub backlisted environment entries, unless they were added explicitly via extraEnv.
    for (let name of ['PYTHONHOME', 'PYTHONPATH', 'CODELLDB_STARTUP']) {
        if (extraEnv[name] === undefined)
            delete env[name];
    }
    return env;
}

// Whether this is an x86 process running on Apple M1 CPU.
export async function isRosetta(): Promise<boolean> {
    return await isRosettaAsync;
}

async function isRosettaImpl(): Promise<boolean> {
    if (os.platform() == 'darwin' && os.arch() == 'x64') {
        let sysctl = await async.cp.execFile('sysctl', ['-in', 'sysctl.proc_translated'], { encoding: 'utf8' });
        return parseInt(sysctl.stdout) == 1;
    } else {
        return false;
    }
}
let isRosettaAsync = isRosettaImpl();
