import * as cp from 'child_process';
import * as path from 'path';
import * as async from './async';
import * as os from 'os';
import { Readable } from 'stream';
import { Dict, Environment } from './commonTypes';
import { mergedEnvironment } from './expand';

export interface AdapterStartOptions {
    extensionRoot: string;
    workDir: string;
    extraEnv: Dict<string>; // extra environment to be set for adapter
    port: number,
    connect: boolean, // Whether to connect or to listen on the port
    adapterParameters: Dict<any>; // feature parameters to pass on to the adapter
    verboseLogging: boolean;
}

export async function start(
    liblldb: string,
    options: AdapterStartOptions
): Promise<cp.ChildProcess> {

    let executable = path.join(options.extensionRoot, 'adapter', 'codelldb');
    let portAction = options.connect ? '--connect' : '--port';
    let args = ['--liblldb', liblldb, portAction, options.port.toString()];
    if (options.adapterParameters) {
        args = args.concat(['--params', JSON.stringify(options.adapterParameters)]);
    }
    let env = getAdapterEnv(options.extraEnv);
    env['RUST_TRACEBACK'] = '1';
    if (options.verboseLogging) {
        env['RUST_LOG'] = 'error,codelldb=debug';
    }

    return spawnDebugAdapter(executable, args, env, options.workDir);
}

export async function spawnDebugAdapter(
    executable: string,
    args: string[],
    env: Environment,
    workDir: string
): Promise<cp.ChildProcess> {
    // Check if workDir exists and is a directory, otherwise launch with default cwd.
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

    return cp.spawn(executable, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: env,
        cwd: workDir
    });
}

export async function findLibLLDB(pathHint: string): Promise<string | null> {
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
    }

    for (let dir of [pathHint, libDir]) {
        let file = await findFileByPattern(dir, pattern);
        if (file) {
            return path.join(dir, file);
        }
    }
    return null;
}

async function findFileByPattern(path: string, pattern: RegExp): Promise<string | null> {
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
    return null;
}

export function getAdapterEnv(extraEnv: Dict<string>): Environment {
    let env = mergedEnvironment(extraEnv);
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
