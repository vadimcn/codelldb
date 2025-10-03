import { LaunchEnvironment, LaunchRequestArguments, LaunchResponse } from 'codelldb';
import * as crypto from 'crypto';
import * as net from 'net';
import * as querystring from 'querystring';
import stringArgv from 'string-argv';
import { debug, DebugConfiguration, EventEmitter, tasks, Uri, UriHandler, window, workspace } from "vscode";
import YAML from 'yaml';
import { output } from './logging';
import { Dict } from "./novsc/commonTypes";

export class UriLaunchServer implements UriHandler {
    async handleUri(uri: Uri) {
        try {
            output.appendLine(`Handling uri: ${uri}`);

            if (uri.path == '/launch') {
                let params = querystring.parse(uri.query, ',') as Dict<string>;
                if (params.folder && params.name) {
                    let wsFolder = workspace.getWorkspaceFolder(Uri.file(params.folder));
                    await debug.startDebugging(wsFolder, params.name);

                } else if (params.name) {
                    if (workspace.workspaceFolders) {
                        // Try all workspace folders
                        for (let wsFolder of workspace.workspaceFolders) {
                            if (await debug.startDebugging(wsFolder, params.name))
                                break;
                        }
                    }
                } else {
                    throw new Error(`Unsupported combination of launch Uri parameters.`);
                }

            } else if (uri.path == '/launch/command') {
                let frags = uri.query.split('&');
                let cmdLine = frags.pop();

                let env: Dict<string> = {}
                for (let frag of frags) {
                    let parts = frag.split('=', 2);
                    env[parts[0]] = parts[1];
                }

                let args = cmdLine ? stringArgv(cmdLine) : [];
                let program = args.shift();
                let debugConfig: DebugConfiguration = {
                    type: 'lldb',
                    request: 'launch',
                    name: '',
                    program: program,
                    args: args,
                    env: env,
                };
                debugConfig.name = debugConfig.name || debugConfig.program;
                await debug.startDebugging(undefined, debugConfig);

            } else if (uri.path == '/launch/config') {
                let debugConfig: DebugConfiguration = {
                    type: 'lldb',
                    request: 'launch',
                    name: '',
                };
                Object.assign(debugConfig, YAML.parse(uri.query));
                debugConfig.name = debugConfig.name || debugConfig.program;
                await debug.startDebugging(undefined, debugConfig);

            } else {
                throw new Error(`Unsupported Uri path: ${uri.path}`);
            }
        } catch (err: any) {
            await window.showErrorMessage(err.toString());
        }
    }
}

export class RpcServer {
    inner: net.Server;
    processRequest: (request: string) => string | Promise<string>;
    errorEmitter = new EventEmitter<Error>();
    readonly onError = this.errorEmitter.event;

    constructor(processRequest: (request: string) => string | Promise<string>) {
        this.processRequest = processRequest;
        this.inner = net.createServer({ allowHalfOpen: true });
        this.inner.on('error', err => this.errorEmitter.fire(err));
        this.inner.on('connection', socket => {
            let request = '';
            socket.on('data', chunk => request += chunk);
            socket.on('end', () => {
                let response = this.processRequest(request);
                if (response instanceof Promise) {
                    response.then(value => socket.end(value));
                } else {
                    socket.end(response);
                }
            });
        });
    }

    public async listen(options: net.ListenOptions) {
        return new Promise<net.AddressInfo | string | null>(resolve =>
            this.inner.listen(options, () => resolve(this.inner.address()))
        );
    }

    public close() {
        this.inner.close();
    }
}

export class RpcLaunchServer extends RpcServer {
    token?: string;

    constructor(options: { token?: string }) {
        super(request => this.onRequest(request).then(response => JSON.stringify(response)));
        this.token = options.token;
    }

    async onRequest(rawRequest: string): Promise<LaunchResponse> {
        let request = YAML.parse(rawRequest);

        let debugConfig: DebugConfiguration & LaunchRequestArguments = {
            type: 'lldb',
            request: 'launch',
            name: '',
            env: {},
            waitEndOfSession: false // Whether to wait for the end of the debug session before responding
        };

        if (request.type == 'LaunchEnvironment') {
            let launchEnv = request as LaunchEnvironment;
            let launchConfig = launchEnv.config ? YAML.parse(launchEnv.config) : {};
            debugConfig.program = launchEnv.cmd[0];
            debugConfig.args = launchEnv.cmd.slice(1);
            debugConfig.terminal = launchEnv.terminalId;
            debugConfig.waitEndOfSession = true;
            Object.assign(debugConfig, launchConfig);
            debugConfig.env = Object.assign(debugConfig.env as any, launchEnv.env, launchConfig.env);
            debugConfig.relativePathBase = launchEnv.cwd;
        } else { // Naked DebugConfiguration
            Object.assign(debugConfig, request);
        }

        debugConfig.name = debugConfig.name || debugConfig.program || '';
        if (this.token) {
            if (debugConfig.token != this.token)
                return { success: false, message: 'Token mismatch' };
            delete debugConfig.token;
        }

        try {
            let endSessionAsync = undefined;
            if (debugConfig.waitEndOfSession) {
                endSessionAsync = waitEndOfDebugSession(debugConfig);
            }
            let success = await debug.startDebugging(undefined, debugConfig);
            if (success && endSessionAsync) {
                success = await endSessionAsync;
            }
            return { success: success };
        } catch (err: any) {
            return { success: false, message: err.toString() };
        }
    };
}


export function waitEndOfDebugSession(debugConfig: DebugConfiguration, timeout: number = 5000): Promise<boolean> {
    let resolvePromise: (value: boolean) => void;
    let promise = new Promise<boolean>(resolve => resolvePromise = resolve);

    let sessionId = crypto.randomBytes(16).toString('base64');
    debugConfig._codelldbSessionId = sessionId;

    let failedLaunchCleanup: NodeJS.Timeout;
    let startSub = debug.onDidStartDebugSession(session => {
        if (session.configuration._codelldbSessionId == sessionId) {
            startSub.dispose();
            clearTimeout(failedLaunchCleanup); // Disarm the cleanup timer.
            let endSub = debug.onDidTerminateDebugSession(session => {
                if (session.configuration._codelldbSessionId == sessionId) {
                    endSub.dispose();
                    resolvePromise(true);
                }
            })
        }
    });

    function armTimer() {
        failedLaunchCleanup = setTimeout(() => {
            startSub.dispose();
            resolvePromise(false)
        }, timeout);
    }

    let preLaunchTask = debugConfig.preLaunchTask as string;
    if (preLaunchTask) {
        let taskSub = tasks.onDidEndTask(e => {
            if (e.execution.task.name == preLaunchTask) {
                taskSub.dispose();
                armTimer();
            }
        });
    } else {
        armTimer();
    }

    return promise;
}
