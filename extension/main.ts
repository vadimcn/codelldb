import {
    workspace, window, commands, debug,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration, DebugAdapterDescriptorFactory, DebugSession, DebugAdapterExecutable,
    DebugAdapterDescriptor, Uri, StatusBarAlignment, QuickPickItem, StatusBarItem, UriHandler, ConfigurationTarget,
    DebugAdapterInlineImplementation
} from 'vscode';
import { inspect } from 'util';
import { ChildProcess } from 'child_process';
import * as path from 'path';
import * as os from 'os';
import * as querystring from 'querystring';
import YAML from 'yaml';
import stringArgv from 'string-argv';
import * as htmlView from './htmlView';
import * as util from './configUtils';
import * as adapter from './novsc/adapter';
import * as install from './install';
import { Cargo, expandCargo } from './cargo';
import { pickProcess } from './pickProcess';
import { Dict } from './novsc/commonTypes';
import { AdapterSettings } from './adapterMessages';
import { ModuleTreeDataProvider } from './modulesView';
import { mergeValues } from './novsc/expand';
import { pickSymbol } from './symbols';
import { ReverseAdapterConnector } from './novsc/reverseConnector';
import { SimpleServer } from './simpleServer';

export let output = window.createOutputChannel('LLDB');

let extension: Extension;

// Main entry point
export function activate(context: ExtensionContext) {
    extension = new Extension(context);
    extension.onActivate();
}

export function deactivate() {
    extension.onDeactivate();
}

class Extension implements DebugConfigurationProvider, DebugAdapterDescriptorFactory, UriHandler {
    context: ExtensionContext;
    htmlViewer: htmlView.DebuggerHtmlView;
    status: StatusBarItem;
    loadedModules: ModuleTreeDataProvider;
    rpcServer: SimpleServer;

    constructor(context: ExtensionContext) {
        this.context = context;
        this.htmlViewer = new htmlView.DebuggerHtmlView(context);

        let subscriptions = context.subscriptions;

        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));
        subscriptions.push(debug.registerDebugAdapterDescriptorFactory('lldb', this));

        subscriptions.push(commands.registerCommand('lldb.diagnose', () => this.runDiagnostics()));
        subscriptions.push(commands.registerCommand('lldb.getCargoLaunchConfigs', () => this.getCargoLaunchConfigs()));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', () => pickProcess(context, false)));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', () => pickProcess(context, true)));
        subscriptions.push(commands.registerCommand('lldb.changeDisplaySettings', () => this.changeDisplaySettings()));
        subscriptions.push(commands.registerCommand('lldb.attach', () => this.attach()));
        subscriptions.push(commands.registerCommand('lldb.alternateBackend', () => this.alternateBackend()));
        subscriptions.push(commands.registerCommand('lldb.commandPrompt', () => this.commandPrompt()));
        subscriptions.push(commands.registerCommand('lldb.symbols', () => pickSymbol(debug.activeDebugSession)));
        subscriptions.push(commands.registerCommand('lldb.viewMemory', () => this.viewMemory()));

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.displayFormat') ||
                event.affectsConfiguration('lldb.showDisassembly') ||
                event.affectsConfiguration('lldb.dereferencePointers') ||
                event.affectsConfiguration('lldb.suppressMissingSourceFiles') ||
                event.affectsConfiguration('lldb.evaluationTimeout') ||
                event.affectsConfiguration('lldb.consoleMode')) {
                this.propagateDisplaySettings();
            }
            if (event.affectsConfiguration('lldb.library')) {
                this.adapterDylibsCache = null;
            }
            if (event.affectsConfiguration('lldb.rpcServer')) {
                this.startRpcServer();
            }
        }));

        this.registerDisplaySettingCommand('lldb.toggleConsoleMode', async (settings) => {
            settings.consoleMode = (settings.consoleMode == 'commands') ? 'evaluate' : 'commands';
        });
        this.registerDisplaySettingCommand('lldb.showDisassembly', async (settings) => {
            settings.showDisassembly = <AdapterSettings['showDisassembly']>await window.showQuickPick(['always', 'auto', 'never']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDisassembly', async (settings) => {
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
        });
        this.registerDisplaySettingCommand('lldb.displayFormat', async (settings) => {
            settings.displayFormat = <AdapterSettings['displayFormat']>await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDerefPointers', async (settings) => {
            settings.dereferencePointers = !settings.dereferencePointers;
        });

        this.status = window.createStatusBarItem(StatusBarAlignment.Left, 0);
        this.status.command = 'lldb.changeDisplaySettings';
        this.status.tooltip = 'Change debugger display settings';
        this.status.hide();

        subscriptions.push(debug.onDidChangeActiveDebugSession(session => {
            if (session && session.type == 'lldb')
                this.status.show();
            else
                this.status.hide();
        }));

        this.loadedModules = new ModuleTreeDataProvider(context);
        subscriptions.push(window.registerTreeDataProvider('loadedModules', this.loadedModules));

        subscriptions.push(window.registerUriHandler(this));

        this.startRpcServer();
    }

    async onActivate() {
        this.propagateDisplaySettings();
        install.ensurePlatformPackage(this.context, output, false);
    }

    onDeactivate() {
        if (this.rpcServer) {
            this.rpcServer.close();
        }
    }

    async handleUri(uri: Uri) {
        try {
            output.appendLine(`Handling uri: ${uri}`);
            let query = decodeURIComponent(uri.query);
            output.appendLine(`Decoded query:\n${query}`);

            if (uri.path == '/launch') {
                let params = <Dict<string>>querystring.parse(uri.query, ',');
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
                let frags = query.split('&');
                let cmdLine = frags.pop();

                let env: Dict<string> = {}
                for (let frag of frags) {
                    let pos = frag.indexOf('=');
                    if (pos > 0)
                        env[frag.substr(0, pos)] = frag.substr(pos + 1);
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
                Object.assign(debugConfig, YAML.parse(query));
                debugConfig.name = debugConfig.name || debugConfig.program;
                await debug.startDebugging(undefined, debugConfig);

            } else {
                throw new Error(`Unsupported Uri path: ${uri.path}`);
            }
        } catch (err) {
            await window.showErrorMessage(err.message);
        }
    }

    startRpcServer() {
        if (this.rpcServer) {
            output.appendLine('Stopping RPC server');
            this.rpcServer.close();
            this.rpcServer = null;
        }

        let config = this.getExtensionConfig()
        let rpcOptions: any = config.get('rpcServer');
        if (rpcOptions) {
            output.appendLine(`Starting RPC server with: ${inspect(rpcOptions)}`);

            this.rpcServer = new SimpleServer({ allowHalfOpen: true });
            this.rpcServer.processRequest = async (request) => {
                let debugConfig: DebugConfiguration = {
                    type: 'lldb',
                    request: 'launch',
                    name: '',
                };
                Object.assign(debugConfig, YAML.parse(request));
                debugConfig.name = debugConfig.name || debugConfig.program;
                if (rpcOptions.token) {
                    if (debugConfig.token != rpcOptions.token)
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
            this.rpcServer.listen(rpcOptions);
        }
    }

    registerDisplaySettingCommand(command: string, updater: (settings: AdapterSettings) => Promise<void>) {
        this.context.subscriptions.push(commands.registerCommand(command, async () => {
            let settings = this.getAdapterSettings();
            await updater(settings);
            this.setAdapterSettings(settings);
        }));
    }

    // Read current adapter settings values from workspace configuration.
    getAdapterSettings(folder: WorkspaceFolder = undefined): AdapterSettings {
        folder = folder || debug.activeDebugSession?.workspaceFolder;
        let config = this.getExtensionConfig(folder);
        let settings: AdapterSettings = {
            displayFormat: config.get('displayFormat'),
            showDisassembly: config.get('showDisassembly'),
            dereferencePointers: config.get('dereferencePointers'),
            suppressMissingSourceFiles: config.get('suppressMissingSourceFiles'),
            evaluationTimeout: config.get('evaluationTimeout'),
            consoleMode: config.get('consoleMode'),
            sourceLanguages: null,
            terminalPromptClear: config.get('terminalPromptClear'),
            evaluateForHovers: config.get('evaluateForHovers'),
            commandCompletions: config.get('commandCompletions'),
            reproducer: config.get('reproducer'),
        };
        return settings;
    }

    // Update workspace configuration.
    async setAdapterSettings(settings: AdapterSettings) {
        let folder = debug.activeDebugSession?.workspaceFolder;
        let config = this.getExtensionConfig(folder);
        await config.update('displayFormat', settings.displayFormat);
        await config.update('showDisassembly', settings.showDisassembly);
        await config.update('dereferencePointers', settings.dereferencePointers);
        await config.update('consoleMode', settings.consoleMode);
    }

    // This is called When configuration change is detected. Updates UI, and if a debug session
    // is active, pushes updated settings to the adapter as well.
    async propagateDisplaySettings() {
        let settings = this.getAdapterSettings();

        this.status.text =
            `Format: ${settings.displayFormat}  ` +
            `Disasm: ${settings.showDisassembly}  ` +
            `Deref: ${settings.dereferencePointers ? 'on' : 'off'}  ` +
            `Console: ${settings.consoleMode == 'commands' ? 'cmd' : 'eval'}`;

        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            await debug.activeDebugSession.customRequest('_adapterSettings', settings);
        }
    }

    // UI for changing display settings.
    async changeDisplaySettings() {
        let settings = this.getAdapterSettings();
        let qpick = window.createQuickPick<QuickPickItem & { command: string }>();
        qpick.items = [
            {
                label: `Value formatting: ${settings.displayFormat}`,
                detail: 'Default format for displaying variable values and evaluation results.',
                command: 'lldb.displayFormat'
            },
            {
                label: `Show disassembly: ${settings.showDisassembly}`,
                detail: 'When to display disassembly.',
                command: 'lldb.showDisassembly'
            },
            {
                label: `Dereference pointers: ${settings.dereferencePointers ? 'on' : 'off'}`,
                detail: 'Whether to show a summary of the pointee or a numeric pointer value.',
                command: 'lldb.toggleDerefPointers'
            },
            {
                label: `Console mode: ${settings.consoleMode}`,
                detail: 'Whether Debug Console input is treated as debugger commands or as expressions to evaluate.',
                command: 'lldb.toggleConsoleMode'
            }
        ];
        qpick.title = 'Debugger display settings';
        qpick.onDidAccept(() => {
            let item = qpick.selectedItems[0];
            qpick.hide();
            commands.executeCommand(item.command);
        });
        qpick.show();
    }

    async provideDebugConfigurations(
        workspaceFolder: WorkspaceFolder | undefined,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        try {
            let cargo = new Cargo(workspaceFolder, cancellation);
            let debugConfigs = await cargo.getLaunchConfigs();
            if (debugConfigs.length > 0) {
                let response = await window.showInformationMessage(
                    'Cargo.toml has been detected in this workspace.\r\n' +
                    'Would you like to generate launch configurations for its targets?', { modal: true }, 'Yes', 'No');
                if (response == 'Yes') {
                    return debugConfigs;
                }
            }
        } catch (err) {
            output.appendLine(err.toString());
        }

        return [{
            type: 'lldb',
            request: 'launch',
            name: 'Debug',
            program: '${workspaceFolder}/<executable file>',
            args: [],
            cwd: '${workspaceFolder}'
        }];
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        launchConfig: DebugConfiguration,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration> {
        output.clear();

        output.appendLine(`Initial debug configuration: ${inspect(launchConfig)}`);

        if (launchConfig.type === undefined) {
            await window.showErrorMessage('Cannot start debugging because no launch configuration has been provided.', { modal: true });
            return null;
        }

        if (!await this.checkPrerequisites(folder))
            return undefined;

        let config = this.getExtensionConfig(folder);

        let launchDefaults = this.getExtensionConfig(folder, 'lldb.launch');
        launchConfig = this.mergeWorkspaceSettings(launchDefaults, launchConfig);

        let dbgconfigConfig = this.getExtensionConfig(folder, 'lldb.dbgconfig');
        launchConfig = util.expandDbgConfig(launchConfig, dbgconfigConfig);

        // Transform "request":"custom" to "request":"launch" + "custom":true
        if (launchConfig.request == 'custom') {
            launchConfig.request = 'launch';
            launchConfig.custom = true;
        }

        if (typeof launchConfig.args == 'string') {
            launchConfig.args = stringArgv(launchConfig.args);
        }

        launchConfig.relativePathBase = launchConfig.relativePathBase || workspace.rootPath;

        // Deal with Cargo
        if (launchConfig.cargo != undefined) {
            let cargo = new Cargo(folder, cancellation);
            let program = await cargo.getProgramFromCargoConfig(launchConfig.cargo);
            delete launchConfig.cargo;

            // Expand ${cargo:program}.
            launchConfig = expandCargo(launchConfig, { program: program });

            if (launchConfig.program == undefined) {
                launchConfig.program = program;
            }

            // Add 'rust' to sourceLanguages, since this project obviously (ha!) involves Rust.
            if (!launchConfig.sourceLanguages)
                launchConfig.sourceLanguages = [];
            launchConfig.sourceLanguages.push('rust');
        }

        launchConfig._adapterSettings = this.getAdapterSettings();
        if (launchConfig.sourceLanguages) {
            launchConfig._adapterSettings.sourceLanguages = launchConfig.sourceLanguages;
            delete launchConfig.sourceLanguages;
        }

        output.appendLine(`Resolved debug configuration: ${inspect(launchConfig)}`);
        return launchConfig;
    }

    async createDebugAdapterDescriptor(session: DebugSession, executable: DebugAdapterExecutable | undefined): Promise<DebugAdapterDescriptor> {
        let settings = this.getAdapterSettings(session.workspaceFolder);
        let adapterParams: any = {
            evaluateForHovers: settings.evaluateForHovers,
            commandCompletions: settings.commandCompletions,
        };
        let connector = new ReverseAdapterConnector();
        let port = await connector.listen();

        try {
            await this.startDebugAdapter(session.workspaceFolder, adapterParams, port);
            await connector.accept();
            return new DebugAdapterInlineImplementation(connector);
        } catch (err) {
            this.analyzeStartupError(err);
            throw err;
        }
    }

    async analyzeStartupError(err: Error) {
        output.appendLine(err.toString());
        output.show(true)
        let e = <any>err;
        let diagnostics = 'Run diagnostics';
        let actionAsync;
        if (e.code == 'ENOENT') {
            actionAsync = window.showErrorMessage(
                `Could not start debugging because executable "${e.path}" was not found.`,
                diagnostics);
        } else if (e.code == 'Timeout' || e.code == 'Handshake') {
            actionAsync = window.showErrorMessage(err.message, diagnostics);
        } else {
            actionAsync = window.showErrorMessage('Could not start debugging.', diagnostics);
        }
        if ((await actionAsync) == diagnostics) {
            await this.runDiagnostics();
        }
    }

    // Merge launch configuration with workspace settings
    mergeWorkspaceSettings(launchConfig: WorkspaceConfiguration, debugConfig: DebugConfiguration): DebugConfiguration {
        let mergeConfig = (key: string, reverse: boolean = false) => {
            let value1 = util.getConfigNoDefault(launchConfig, key);
            let value2 = debugConfig[key];
            let value = !reverse ? mergeValues(value1, value2) : mergeValues(value2, value1);
            if (!util.isEmpty(value))
                debugConfig[key] = value;
        }
        mergeConfig('initCommands');
        mergeConfig('preRunCommands');
        mergeConfig('postRunCommands');
        mergeConfig('exitCommands', true);
        mergeConfig('env');
        mergeConfig('cwd');
        mergeConfig('terminal');
        mergeConfig('stdio');
        mergeConfig('expressions');
        mergeConfig('sourceMap');
        mergeConfig('relativePathBase');
        mergeConfig('sourceLanguages');
        mergeConfig('debugServer');
        return debugConfig;
    }

    async getCargoLaunchConfigs() {
        try {
            let folder = (workspace.workspaceFolders.length == 1) ?
                workspace.workspaceFolders[0] :
                await window.showWorkspaceFolderPick();
            let cargo = new Cargo(folder);
            let configurations = await cargo.getLaunchConfigs();
            let debugConfigs = {
                version: '0.2.0',
                configurations: configurations,
            }
            let doc = await workspace.openTextDocument({
                language: 'jsonc',
                content: JSON.stringify(debugConfigs, null, 4),
            });
            await window.showTextDocument(doc, 1, false);
        } catch (err) {
            output.show();
            window.showErrorMessage(err.toString());
        }
    }

    async startDebugAdapter(
        folder: WorkspaceFolder | undefined,
        adapterParams: Dict<string>,
        connectPort: number
    ): Promise<ChildProcess> {
        let config = this.getExtensionConfig(folder);
        let adapterEnv = config.get('adapterEnv', {});
        let verboseLogging = config.get<boolean>('verboseLogging');
        let [liblldb] = await this.getAdapterDylibs(config);

        if (verboseLogging) {
            output.appendLine(`liblldb: ${liblldb}`);
            output.appendLine(`environment: ${inspect(adapterEnv)}`);
            output.appendLine(`params: ${inspect(adapterParams)}`);
        }

        let adapterProcess = await adapter.start(liblldb, {
            extensionRoot: this.context.extensionPath,
            extraEnv: adapterEnv,
            workDir: workspace.rootPath,
            port: connectPort,
            connect: true,
            adapterParameters: adapterParams,
            verboseLogging: verboseLogging
        });

        util.logProcessOutput(adapterProcess, output);

        adapterProcess.on('exit', async (code, signal) => {
            output.appendLine(`Debug adapter exit code=${code} (0x${code.toString(16)}), signal=${signal}.`);
            if (code != 0) {
                let result = await window.showErrorMessage('Oops!  The debug adapter has terminated abnormally.', 'Open log');
                if (result != undefined) {
                    output.show();
                }
            }
        });
        return adapterProcess;
    }

    // Resolve paths of the native adapter libraries and cache them.
    async getAdapterDylibs(config: WorkspaceConfiguration): Promise<[string]> {
        if (!this.adapterDylibsCache) {
            let liblldb = config.get<string>('library');
            if (liblldb) {
                liblldb = await adapter.findLibLLDB(liblldb)
            } else {
                liblldb = await adapter.findLibLLDB(path.join(this.context.extensionPath, 'lldb'));
            }
            this.adapterDylibsCache = [liblldb];
        }
        return this.adapterDylibsCache;
    }
    adapterDylibsCache: [string] = null;

    async checkPrerequisites(folder?: WorkspaceFolder): Promise<boolean> {
        if (!await install.ensurePlatformPackage(this.context, output, true))
            return false;
        return true;
    }

    async runDiagnostics(folder?: WorkspaceFolder) {
        let succeeded;
        try {
            let connector = new ReverseAdapterConnector();
            let port = await connector.listen();
            let adapter = await this.startDebugAdapter(folder, {}, port);
            let adapterExitAsync = new Promise((resolve, reject) => {
                adapter.on('exit', resolve);
                adapter.on('error', reject);
            });
            await connector.accept();
            connector.handleMessage({ seq: 1, type: 'request', command: 'disconnect' });
            connector.dispose();
            await adapterExitAsync;
            succeeded = true;
        } catch (err) {
            succeeded = false;
        }

        if (succeeded) {
            window.showInformationMessage('LLDB self-test completed successfuly.');
        } else {
            window.showErrorMessage('LLDB self-test has failed.  Please check log output.');
            output.show();
        }
    }

    async attach() {
        let debugConfig: DebugConfiguration = {
            type: 'lldb',
            request: 'attach',
            name: 'Attach',
            pid: '${command:pickMyProcess}',
        };
        await debug.startDebugging(undefined, debugConfig);
    }

    async alternateBackend() {
        let box = window.createInputBox();
        box.prompt = 'Enter file name of the LLDB instance you\'d like to use. ';
        box.onDidAccept(async () => {
            try {
                let dirs = await util.getLLDBDirectories(box.value);
                if (dirs) {
                    let libraryPath = await adapter.findLibLLDB(dirs.shlibDir);
                    if (libraryPath) {
                        let choice = await window.showInformationMessage(
                            `Located liblldb at: ${libraryPath}\r\nUse it to configure the current workspace?`,
                            { modal: true }, 'Yes'
                        );
                        if (choice == 'Yes') {
                            box.hide();
                            let lldbConfig = this.getExtensionConfig();
                            lldbConfig.update('library', libraryPath, ConfigurationTarget.Workspace);
                        } else {
                            box.show();
                        }
                    }
                }
            } catch (err) {
                let message = (err.code == 'ENOENT') ? `could not find "${err.path}".` : err.message;
                await window.showErrorMessage(`Failed to query LLDB for library location: ${message}`, { modal: true });
                box.show();
            }
        });
        box.show();
    }

    commandPrompt() {
        let lldb = os.platform() != 'win32' ? 'lldb' : 'lldb.exe';
        let lldbPath = path.join(this.context.extensionPath, 'lldb', 'bin', lldb);
        let consolePath = path.join(this.context.extensionPath, 'adapter', 'console.py');
        let folder = workspace.workspaceFolders[0];
        let config = this.getExtensionConfig(folder);
        let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));

        let terminal = window.createTerminal({
            name: 'LLDB Command Prompt',
            shellPath: lldbPath,
            shellArgs: ['--no-lldbinit', '--one-line-before-file', 'command script import ' + consolePath],
            cwd: folder.uri.fsPath,
            env: env,
            strictEnv: true
        });
        terminal.show()
    }

    async viewMemory(address?: bigint) {
        if (address == undefined) {
            let addressStr = await window.showInputBox({
                title: 'Enter memory address',
                prompt: 'Hex, octal or decmal '
            });
            try {
                address = BigInt(addressStr);
            } catch (error) {
                window.showErrorMessage('Could not parse address', { modal: true });
                return;
            }
        }
        commands.executeCommand('workbench.debug.viewlet.action.viewMemory', {
            sessionId: debug.activeDebugSession.id,
            variable: {
                memoryReference: `0x${address.toString(16)}`
            }
        });
    }

    getExtensionConfig(folder?: WorkspaceFolder, key: string = 'lldb'): WorkspaceConfiguration {
        return workspace.getConfiguration(key, folder?.uri);
    }
}


