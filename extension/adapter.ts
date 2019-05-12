import * as cp from 'child_process';
import * as path from 'path';
import { Readable } from 'stream';
import * as util from './util';
import { Dict } from './common';
import { statAsync } from './async';
import { Environment } from './util';

export async function startClassic(
    extensionRoot: string,
    lldbLocation: string,
    extraEnv: Dict<string>, // extra environment to be set for adapter
    workDir: string,
    adapterParameters: Dict<any>, // feature parameters that should be passed on to the adapter
    verboseLogging: boolean,
): Promise<cp.ChildProcess> {

    let env = util.mergeEnv(extraEnv);
    if (verboseLogging) {
        adapterParameters['logLevel'] = 0;
    }
    let paramsBase64 = new Buffer(JSON.stringify(adapterParameters)).toString('base64');
    let args = ['-b',
        '-O', `command script import '${path.join(extensionRoot, 'adapter')}'`,
        '-O', `script adapter.run_tcp_session(0, '${paramsBase64}')`
    ];
    return spawnDebugAdapter(lldbLocation, args, env, workDir);
}

export async function startNative(
    extensionRoot: string,
    liblldb: string,
    extraEnv: Dict<string>, // extra environment to be set for adapter
    workDir: string,
    adapterParameters: Dict<any>, // feature parameters that should be passed on to the adapter
    verboseLogging: boolean,
): Promise<cp.ChildProcess> {

    let env = util.mergeEnv(extraEnv);
    let executable = path.join(extensionRoot, 'adapter2/codelldb');
    let args = ['--preload', liblldb];
    if (process.platform == 'win32') {
        // Add liblldb's directory to PATH so it can find msdia dll later.
        env['PATH'] = env['PATH'] + ';' + path.dirname(liblldb);
        // LLDB will need python36.dll anyways, and we can provide a better error message
        // if we preload it explicitly.
        args = ['--preload', 'python36.dll'].concat(args);
    }
    if (adapterParameters) {
        args = args.concat(['--params', JSON.stringify(adapterParameters)]);
    }
    if (verboseLogging) {
        env['RUST_LOG'] = 'error,codelldb=debug';
        env['RUST_TRACEBACK'] = '1';
    }
    return spawnDebugAdapter(executable, args, env, workDir);
}

export async function spawnDebugAdapter(
    executable: string,
    args: string[],
    env: Environment,
    cwd: string
): Promise<cp.ChildProcess> {
    if (process.platform == 'darwin') {
        // Make sure LLDB finds system Python before Brew Python
        // https://github.com/Homebrew/legacy-homebrew/issues/47201
        env['PATH'] = '/usr/bin:' + env['PATH'];
    } else if (process.platform == 'win32') {
        // Try to locate Python installation and add it to the PATH.
        let pythonPath = await getWindowsPythonPath();
        if (pythonPath) {
            env['PATH'] = env['PATH'] + ';' + pythonPath;
        }
    }

    return cp.spawn(executable, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: env,
        cwd: cwd
    });
}

export async function getDebugServerPort(adapter: cp.ChildProcess): Promise<number> {
    let regex = /^Listening on port (\d+)\s/m;
    let match = await waitForPattern(adapter, adapter.stdout, regex);
    return parseInt(match[1]);
}

export function waitForPattern(
    process: cp.ChildProcess,
    channel: Readable,
    pattern: RegExp,
    timeoutMillis = 10000
): Promise<RegExpExecArray> {
    return new Promise<RegExpExecArray>((resolve, reject) => {
        let promisePending = true;
        let processOutput = '';
        // Wait for expected pattern in channel.
        channel.on('data', chunk => {
            let chunkStr = chunk.toString();
            if (promisePending) {
                processOutput += chunkStr;
                let match = pattern.exec(processOutput);
                if (match) {
                    clearTimeout(timer);
                    processOutput = null;
                    promisePending = false;
                    resolve(match);
                }
            }
        });
        // On spawn error.
        process.on('error', err => {
            promisePending = false;
            reject(err);
        });
        // Bail if LLDB does not start within the specified timeout.
        let timer = setTimeout(() => {
            if (promisePending) {
                process.kill();
                let err = Error('The debugger did not start within the allotted time.');
                (<any>err).code = 'Timeout';
                (<any>err).stdout = processOutput;
                promisePending = false;
                reject(err);
            }
        }, timeoutMillis);
        // Premature exit.
        process.on('exit', (code, signal) => {
            if (promisePending) {
                let err = Error('The debugger exited without completing startup handshake.');
                (<any>err).code = 'Handshake';
                (<any>err).stdout = processOutput;
                promisePending = false;
                reject(err);
            }
        });
    });
}

export async function findLibLLDB(pathHint: string): Promise<string | null> {
    let stat = await statAsync(pathHint);
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
        let file = await util.findFileByPattern(dir, pattern);
        if (file) {
            return path.join(dir, file);
        }
    }
    return null;
}

export const pythonVersion = '3.6';

export async function getWindowsPythonPath(): Promise<string> {
    if (process.platform != 'win32')
        throw new Error('Windows only!');

    let path = await getPythonPathAsync;
    if (path == null) { // Don't cache negative result - in case they install Python without restarting VSCode.
        getPythonPathAsync = getWindowsPythonPathImpl();
        path = await getPythonPathAsync
    }
    return path;
}

// Kick off this query as soon as the module gets loaded.
let getPythonPathAsync = getWindowsPythonPathImpl();

async function getWindowsPythonPathImpl(): Promise<string> {
    if (process.platform != 'win32')
        return undefined;

    let path = await util.readRegistry(`HKCU\\Software\\Python\\PythonCore\\${pythonVersion}\\InstallPath`, null);
    if (!path) {
        path = await util.readRegistry(`HKLM\\Software\\Python\\PythonCore\\${pythonVersion}\\InstallPath`, null);
    }
    return path;
}
