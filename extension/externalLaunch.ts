import { workspace, debug, window, DebugConfiguration, EventEmitter, Uri, UriHandler, } from "vscode";
import * as querystring from 'querystring';
import stringArgv from 'string-argv';
import YAML from 'yaml';
import { Dict } from "./novsc/commonTypes";
import { output } from "./main";
import * as net from 'net';

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
                    // Try all workspace folders
                    for (let wsFolder of workspace.workspaceFolders) {
                        if (await debug.startDebugging(wsFolder, params.name))
                            break;
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

                let args = stringArgv(cmdLine);
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
        } catch (err) {
            await window.showErrorMessage(err.message);
        }
    }
}

export class RpcLaunchServer {
    inner: net.Server;
    token: string;
    errorEmitter = new EventEmitter<Error>();
    readonly onError = this.errorEmitter.event;

    constructor(options: { token?: string }) {
        this.token = options.token;
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

    async processRequest(request: string) {
        let debugConfig: DebugConfiguration = {
            type: 'lldb',
            request: 'launch',
            name: '',
        };
        Object.assign(debugConfig, YAML.parse(request));
        debugConfig.name = debugConfig.name || debugConfig.program;
        if (this.token) {
            if (debugConfig.token != this.token)
                return '';
            delete debugConfig.token;
        }
        try {
            let success = await debug.startDebugging(undefined, debugConfig);
            return JSON.stringify({ success: success });
        } catch (err) {
            return JSON.stringify({ success: false, message: err.toString() });
        }
    };

    public async listen(options: net.ListenOptions) {
        return new Promise<net.AddressInfo | string>(resolve =>
            this.inner.listen(options, () => resolve(this.inner.address()))
        );
    }

    public close() {
        this.inner.close();
    }
}
