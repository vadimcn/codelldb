import {
    workspace, window, commands, debug, extensions,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken, ConfigurationScope,
    DebugConfiguration, DebugAdapterDescriptorFactory, DebugSession, DebugAdapterExecutable,
    DebugAdapterDescriptor, Uri, ConfigurationTarget, DebugAdapterInlineImplementation, DebugConfigurationProviderTriggerKind,
    languages
} from 'vscode';
import { inspect } from 'util';
import { ChildProcess } from 'child_process';
import * as path from 'path';
import * as os from 'os';
import * as crypto from 'crypto';
import stringArgv from 'string-argv';
import { AdapterSettings } from 'codelldb';
import * as webview from './webview';
import * as util from './configUtils';
import * as adapter from './novsc/adapter';
import * as install from './install';
import { Cargo } from './cargo';
import { pickProcess } from './pickProcess';
import { ModuleTreeDataProvider as ModulesView } from './modulesView';
import { ExcludedCallersView } from './excludedCallersView';
import { mergeValues } from './novsc/expand';
import { pickSymbol } from './symbols';
import { ReverseAdapterConnector } from './novsc/reverseConnector';
import { UriLaunchServer, RpcLaunchServer } from './externalLaunch';
import { AdapterSettingsManager } from './adapterSettingsManager';
import { LaunchCompletionProvider } from './launchCompletions';

export let output = window.createOutputChannel('LLDB', 'log');

export function getExtensionConfig(scope?: ConfigurationScope, subkey?: string): WorkspaceConfiguration {
    let key = 'lldb';
    if (subkey) key += '.' + subkey;
    return workspace.getConfiguration(key, scope);
}

let extension: Extension;

// Main entry point
export function activate(context: ExtensionContext) {
    extension = new Extension(context);
    extension.onActivate();
}

export function deactivate() {
    extension.onDeactivate();
}

class Extension implements DebugAdapterDescriptorFactory {
    context: ExtensionContext;
    settingsManager: AdapterSettingsManager;
    webviewManager: webview.WebviewManager;
    loadedModules: ModulesView;
    excludedCallers: ExcludedCallersView;
    rpcServer?: RpcLaunchServer;

    constructor(context: ExtensionContext) {
        this.context = context;

        let subscriptions = context.subscriptions;

        // Register twice, as we'd like to provide configurations for both trigger types.
        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));
        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', {
            provideDebugConfigurations: (folder, token) => this.provideDebugConfigurations(folder, token),
        }, DebugConfigurationProviderTriggerKind.Dynamic));

        let completionProvider = new LaunchCompletionProvider((folder, token) => this.getLaunchLessConfig(folder, token));
        subscriptions.push(languages.registerCompletionItemProvider({ language: 'json' }, completionProvider));
        subscriptions.push(languages.registerCompletionItemProvider({ language: 'jsonc' }, completionProvider));
        subscriptions.push(commands.registerCommand('lldb.insertDebugConfig',
            (...args) => completionProvider.insertDebugConfig(args)));

        subscriptions.push(debug.registerDebugAdapterDescriptorFactory('lldb', this));

        subscriptions.push(commands.registerCommand('lldb.diagnose', () => this.runDiagnostics()));
        subscriptions.push(commands.registerCommand('lldb.getCargoLaunchConfigs', () => this.getCargoLaunchConfigs()));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', (config) => pickProcess(context, false, config)));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', (config) => pickProcess(context, true, config)));
        subscriptions.push(commands.registerCommand('lldb.attach', () => this.attach()));
        subscriptions.push(commands.registerCommand('lldb.alternateBackend', () => this.alternateBackend()));
        subscriptions.push(commands.registerCommand('lldb.commandPrompt', () => this.commandPrompt()));
        subscriptions.push(commands.registerCommand('lldb.symbols', () => pickSymbol(debug.activeDebugSession)));
        subscriptions.push(commands.registerCommand('lldb.viewMemory', () => this.viewMemory()));

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.library')) {
                this.liblldbPath = undefined;
            }
            if (event.affectsConfiguration('lldb.rpcServer')) {
                this.updateRpcServer();
            }
        }));

        this.webviewManager = new webview.WebviewManager();
        subscriptions.push(this.webviewManager);

        this.settingsManager = new AdapterSettingsManager();
        subscriptions.push(this.settingsManager);

        this.loadedModules = new ModulesView();
        subscriptions.push(this.loadedModules);
        subscriptions.push(window.registerTreeDataProvider('lldb.loadedModules', this.loadedModules));

        this.excludedCallers = new ExcludedCallersView(context);
        this.excludedCallers.loadState();
        subscriptions.push(this.excludedCallers);
        subscriptions.push(window.registerTreeDataProvider('lldb.excludedCallers', this.excludedCallers));

        subscriptions.push(window.registerUriHandler(new UriLaunchServer()));

        this.updateRpcServer();
    }

    async onActivate() {
        let pkg = extensions.getExtension('vadimcn.vscode-lldb')!.packageJSON;
        let currVersion = pkg.version;
        let lastVersion = this.context.globalState.get('lastLaunchedVersion');
        let lldbConfig = getExtensionConfig();
        if (currVersion != lastVersion && !lldbConfig.get('suppressUpdateNotifications')) {
            this.context.globalState.update('lastLaunchedVersion', currVersion);
            if (lastVersion != undefined) {
                let buttons = ['What\'s new?', 'Don\'t show this again'];
                let choice = await window.showInformationMessage('CodeLLDB extension has been updated', ...buttons);
                if (choice === buttons[0]) {
                    let changelog = path.join(this.context.extensionPath, 'CHANGELOG.md')
                    let uri = Uri.file(changelog);
                    await commands.executeCommand('markdown.showPreview', uri, null, { locked: true });
                } else if (choice == buttons[1]) {
                    lldbConfig.update('suppressUpdateNotifications', true, ConfigurationTarget.Global);
                }
            }
        }
        install.ensurePlatformPackage(this.context, output, false);
    }

    onDeactivate() {
        if (this.rpcServer) {
            this.rpcServer.close();
        }
    }

    async updateRpcServer() {
        if (this.rpcServer) {
            output.appendLine('Stopping RPC server');
            this.rpcServer.close();
            this.rpcServer = undefined;
        }
        let config = getExtensionConfig();
        let options = config.get<any>('rpcServer');
        if (options) {
            output.appendLine(`Starting RPC server with: ${inspect(options)}`);
            this.rpcServer = new RpcLaunchServer({ token: options.token });
            await this.rpcServer.listen(options);
        }
    }

    // Called when:
    // 1. User creates launch.json
    // 2. User executes "Debug: Select and Start Debugging" command
    async provideDebugConfigurations(
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        let cargo = new Cargo(this.context, workspaceFolder, cancellation);
        let debugConfigs = await cargo.getLaunchConfigs();
        return debugConfigs;
    }

    // Called when debugging starts without a launch.json file
    async getLaunchLessConfig(
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration | undefined> {
        let configs = await this.provideDebugConfigurations(workspaceFolder, cancellation);
        if (configs.length == 0)
            return undefined;
        let items = configs.map(cfg => ({ label: cfg.name, config: cfg }));
        let selection = await window.showQuickPick(items, { title: 'Choose debugging target' }, cancellation);
        return selection?.config;
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        debugConfig: DebugConfiguration,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration | undefined | null> {
        output.clear();

        let config = getExtensionConfig(folder);
        let verboseLogging = config.get<boolean>('verboseLogging');
        output.appendLine(`Verbose logging: ${verboseLogging ? 'on' : 'off'}  (Use "lldb.verboseLogging" setting to change)`);
        output.appendLine(`Platform: ${process.platform} ${process.arch}`);

        output.appendLine(`Initial debug configuration: ${inspect(debugConfig)}`);

        if (debugConfig.type === undefined) {
            let config = await this.getLaunchLessConfig(folder, cancellation);
            if (!config) {
                await window.showErrorMessage('No debug configuration was provided.', { modal: true });
                return null;
            }
            debugConfig = config;
        }

        if (!await this.checkPrerequisites(folder))
            return undefined;

        let launchDefaults = getExtensionConfig(folder, 'launch');
        this.mergeWorkspaceSettings(debugConfig, launchDefaults);

        let dbgconfigConfig = getExtensionConfig(folder, 'dbgconfig');
        debugConfig = util.expandDbgConfig(debugConfig, dbgconfigConfig);

        // Transform "request":"custom" to "request":"launch"
        if (debugConfig.request == 'custom') {
            debugConfig.request = 'launch';
        }

        if (typeof debugConfig.args == 'string') {
            debugConfig.args = stringArgv(debugConfig.args);
        }

        if (debugConfig.cargo) {
            let cargo = new Cargo(this.context, folder, cancellation);
            debugConfig = await cargo.resolveCargoConfig(debugConfig);
        }

        debugConfig.relativePathBase = debugConfig.relativePathBase || folder?.uri.fsPath || workspace.rootPath;
        debugConfig._adapterSettings = this.settingsManager.getAdapterSettings(folder);

        output.appendLine(`Resolved debug configuration: ${inspect(debugConfig)}`);
        return debugConfig;
    }

    async createDebugAdapterDescriptor(session: DebugSession, executable: DebugAdapterExecutable | undefined): Promise<DebugAdapterDescriptor> {
        let settings = this.settingsManager.getAdapterSettings(session.workspaceFolder);
        let adapterSettings: AdapterSettings = {
            evaluateForHovers: settings.evaluateForHovers,
            commandCompletions: settings.commandCompletions,
        };
        if (session.configuration.sourceLanguages) {
            adapterSettings.sourceLanguages = session.configuration.sourceLanguages;
            delete session.configuration.sourceLanguages;
        }

        let authToken = crypto.randomBytes(16).toString('base64');
        let connector = new ReverseAdapterConnector(authToken);
        let port = await connector.listen();

        try {
            await this.startDebugAdapter(session.workspaceFolder, adapterSettings, port, authToken);
            await connector.accept();
            return new DebugAdapterInlineImplementation(connector);
        } catch (err: any) {
            this.analyzeStartupError(err);
            throw err;
        }
    }

    async analyzeStartupError(err: any) {
        output.appendLine(err.toString());
        output.show(true)
        let diagnostics = 'Run diagnostics';
        let actionAsync;
        if (err.code == 'ENOENT') {
            actionAsync = window.showErrorMessage(
                `Could not start debugging because executable "${err.path}" was not found.`,
                diagnostics);
        } else if (err.code == 'Timeout' || err.code == 'Handshake') {
            actionAsync = window.showErrorMessage(err.message, diagnostics);
        } else {
            actionAsync = window.showErrorMessage('Could not start debugging.', diagnostics);
        }
        if ((await actionAsync) == diagnostics) {
            await this.runDiagnostics();
        }
    }

    // Merge workspace launch defaults into debug configuration.
    mergeWorkspaceSettings(debugConfig: DebugConfiguration, launchConfig: WorkspaceConfiguration) {
        let mergeConfig = (key: string, reverseSeq: boolean = false) => {
            let launchValue = debugConfig[key];
            let defaultValue = launchConfig.get(key);
            let value = mergeValues(launchValue, defaultValue, reverseSeq);
            if (!util.isEmpty(value))
                debugConfig[key] = value;
        }
        mergeConfig('initCommands');
        mergeConfig('preRunCommands');
        mergeConfig('postRunCommands');
        mergeConfig('preTerminateCommands', true);
        mergeConfig('exitCommands', true);
        mergeConfig('env');
        mergeConfig('envFile');
        mergeConfig('cwd');
        mergeConfig('terminal');
        mergeConfig('stdio');
        mergeConfig('expressions');
        mergeConfig('sourceMap');
        mergeConfig('relativePathBase');
        mergeConfig('sourceLanguages');
        mergeConfig('debugServer');
        mergeConfig('breakpointMode');
    }

    async getCargoLaunchConfigs() {
        try {
            let folder = (workspace.workspaceFolders?.length == 1) ? workspace.workspaceFolders[0] :
                await window.showWorkspaceFolderPick();
            let cargo = new Cargo(this.context, folder);
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
        } catch (err: any) {
            output.show();
            window.showErrorMessage(err.toString());
        }
    }

    async startDebugAdapter(
        folder: WorkspaceFolder | undefined,
        adapterSettings: AdapterSettings,
        connectPort: number,
        authToken: string
    ): Promise<ChildProcess> {
        let config = getExtensionConfig(folder);
        let adapterEnv = config.get<any>('adapterEnv', {});
        let verboseLogging = config.get<boolean>('verboseLogging', false);
        if (config.get<boolean>('useNativePDBReader'))
            adapterEnv['LLDB_USE_NATIVE_PDB_READER'] = 'true';
        let liblldb = await this.getLibLLDB(config);

        output.appendLine('Launching adapter');
        output.appendLine(`liblldb: ${liblldb}`);
        output.appendLine(`environment: ${inspect(adapterEnv)}`);
        output.appendLine(`settings: ${inspect(adapterSettings)}`);

        let adapterProcess = await adapter.start({
            extensionRoot: this.context.extensionPath,
            liblldb: liblldb,
            extraEnv: adapterEnv,
            workDir: workspace.rootPath,
            port: connectPort,
            connect: true,
            authToken: authToken,
            adapterSettings: adapterSettings,
            verboseLogging: verboseLogging
        });

        util.logProcessOutput(adapterProcess, output);

        adapterProcess.on('exit', async (code, signal) => {
            output.appendLine(`Debug adapter exit code=${code}, signal=${signal}.`);
            if (code != 0) {
                let result = await window.showErrorMessage('Oops!  The debug adapter has terminated abnormally.', 'Open log');
                if (result != undefined) {
                    output.show();
                }
            }
        });
        return adapterProcess;
    }

    // Resolve the path to liblldb and cache it.
    async getLibLLDB(config: WorkspaceConfiguration): Promise<string | undefined> {
        if (!this.liblldbPath) {
            let library = config.get<string>('library');
            if (library) {
                this.liblldbPath = await adapter.findLibLLDB(library)
            } else {
                this.liblldbPath = await adapter.findLibLLDB(path.join(this.context.extensionPath, 'lldb'));
            }
        }
        return this.liblldbPath;
    }
    liblldbPath: string | undefined;

    async checkPrerequisites(folder?: WorkspaceFolder): Promise<boolean> {
        if (!await install.ensurePlatformPackage(this.context, output, true))
            return false;
        return true;
    }

    async runDiagnostics(folder?: WorkspaceFolder) {
        let succeeded;
        try {
            let authToken = crypto.randomBytes(16).toString('base64');
            let connector = new ReverseAdapterConnector(authToken);
            let port = await connector.listen();
            let adapter = await this.startDebugAdapter(folder, {}, port, authToken);
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
            window.showInformationMessage('LLDB self-test completed successfuly.', { modal: true });
        } else {
            window.showErrorMessage('LLDB self-test has failed.  Please check log output.', { modal: true });
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
                            let lldbConfig = getExtensionConfig();
                            lldbConfig.update('library', libraryPath, ConfigurationTarget.Workspace);
                        } else {
                            box.show();
                        }
                    }
                }
            } catch (err: any) {
                let message = (err?.code == 'ENOENT') ? `could not find "${err.path}".` : err.message;
                await window.showErrorMessage(`Failed to query LLDB for library location: ${message}`, { modal: true });
                box.show();
            }
        });
        box.show();
    }

    commandPrompt() {
        let lldb = os.platform() != 'win32' ? 'lldb' : 'lldb.exe';
        let lldbPath = path.join(this.context.extensionPath, 'lldb', 'bin', lldb);
        let consolePath = path.join(this.context.extensionPath, 'adapter', 'scripts', 'console.py');
        let folder = workspace.workspaceFolders?.[0];
        let config = getExtensionConfig(folder);
        let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));

        let terminal = window.createTerminal({
            name: 'LLDB Command Prompt',
            shellPath: lldbPath,
            shellArgs: ['--no-lldbinit', '--one-line-before-file', 'command script import ' + consolePath],
            cwd: folder?.uri,
            env: env,
            strictEnv: true
        });
        terminal.show()
    }

    async viewMemory(address?: bigint) {
        if (!debug.activeDebugSession)
            return;
        if (address == undefined) {
            let addressStr = await window.showInputBox({
                title: 'Enter memory address',
                prompt: 'Hex, octal or decimal '
            });
            try {
                address = BigInt(addressStr!);
            } catch (err) {
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
}
