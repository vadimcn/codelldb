import * as cp from 'child_process';
import * as path from 'path';
import { Readable } from 'stream';
import * as async from './async';
import { Dict, Environment } from './commonTypes';
import { expandVariables } from './expand';

export interface AdapterStartOptions {
    extensionRoot: string;
    workDir: string;
    extraEnv: Dict<string>; // extra environment to be set for adapter
    adapterParameters: Dict<any>; // feature parameters to pass on to the adapter
    verboseLogging: boolean;
}

export async function startClassic(
    lldbExecutable: string,
    options: AdapterStartOptions
): Promise<cp.ChildProcess> {

    let env = mergeEnv(options.extraEnv);
    if (options.verboseLogging) {
        options.adapterParameters['logLevel'] = 0;
    }
    let paramsBase64 = new Buffer(JSON.stringify(options.adapterParameters)).toString('base64');
    let args = ['-b',
        '-O', `command script import '${path.join(options.extensionRoot, 'adapter')}'`,
        '-O', `script adapter.run_tcp_session(0, '${paramsBase64}')`
    ];
    return spawnDebugAdapter(lldbExecutable, args, env, options.workDir);
}

export async function startNative(
    lldbLibrary: string,
    options: AdapterStartOptions
): Promise<cp.ChildProcess> {

    let env = mergeEnv(options.extraEnv);
    let executable = path.join(options.extensionRoot, 'adapter2/codelldb');
    let args = ['--liblldb', lldbLibrary];
    if (process.platform == 'win32') {
        // Add liblldb's directory to PATH so it can find msdia dll later.
        env['PATH'] = env['PATH'] + ';' + path.dirname(lldbLibrary);
    }
    if (options.adapterParameters) {
        args = args.concat(['--params', JSON.stringify(options.adapterParameters)]);
    }
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

    // Check if workDir exists and is a directory, otherwise launch with default cwd.
    if (workDir) {
        let stat = await async.fs.stat(workDir).catch(_ => null);
        if (!stat || !stat.isDirectory())
            workDir = undefined;
    }

    return cp.spawn(executable, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: env,
        cwd: workDir
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


// Expand ${env:...} placeholders in extraEnv and merge it with the current process' environment.
export function mergeEnv(extraEnv: Dict<string>): Environment {
    let env = new Environment(process.platform == 'win32');
    env = Object.assign(env, process.env);
    for (let key in extraEnv) {
        env[key] = expandVariables(extraEnv[key], (type, key) => {
            if (type == 'env')
                return process.env[key];
            throw new Error('Unknown variable type ' + type);
        });
    }
    return env;
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

    let path = await readRegistry(`HKCU\\Software\\Python\\PythonCore\\${pythonVersion}\\InstallPath`, null);
    if (!path) {
        path = await readRegistry(`HKLM\\Software\\Python\\PythonCore\\${pythonVersion}\\InstallPath`, null);
    }
    return path;
}

async function readRegistry(path: string, value?: string): Promise<string> {
    let args = ['query', path];
    if (value != null)
        args.push('/v', value);
    else
        args.push('/ve');
    args.push('/reg:64');

    try {
        let { stdout } = await async.cp.execFile('reg.exe', args);
        let m = (/REG_SZ\s+(.*)/).exec(stdout);
        if (m) {
            return m[1];
        } else {
            return null;
        }
    } catch (err) {
        return null;
    }
}
