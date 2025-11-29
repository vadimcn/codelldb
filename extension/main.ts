import {
    workspace, window, commands, debug, extensions, languages, lm,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken, ConfigurationScope,
    DebugConfiguration, DebugAdapterDescriptorFactory, DebugSession, DebugAdapterExecutable,
    DebugAdapterDescriptor, Uri, ConfigurationTarget, DebugAdapterInlineImplementation, DebugConfigurationProviderTriggerKind,
} from 'vscode';
import { inspect } from 'util';
import * as path from 'node:path';
import * as crypto from 'node:crypto';
import { AddressInfo } from 'node:net';
import stringArgv from 'string-argv';
import { AdapterSettings } from 'codelldb';
import * as webview from './webview';
import * as util from './configUtils';
import * as adapter from './novsc/adapter';
import * as install from './install';
import * as async from './novsc/async';
import { Dict } from './novsc/commonTypes';
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
import { output, showErrorWithLog } from './logging';
import { LLDBCommandTool, SessionInfoTool } from './vibeDebug';
import { alternateBackend, selfTest, commandPrompt } from './adapterUtils';

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
    rpcFile?: string;

    constructor(context: ExtensionContext) {
        this.context = context;

        let subscriptions = context.subscriptions;

        // Register twice, as we'd like to provide configurations for both trigger types.
        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));
        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', {
            provideDebugConfigurations: (folder, token) =>
                this.provideDebugConfigurations(folder, token, DebugConfigurationProviderTriggerKind.Dynamic),
        }, DebugConfigurationProviderTriggerKind.Dynamic));

        let completionProvider = new LaunchCompletionProvider((folder, token) => this.getLaunchLessConfig(folder, token));
        subscriptions.push(languages.registerCompletionItemProvider({ language: 'json' }, completionProvider));
        subscriptions.push(languages.registerCompletionItemProvider({ language: 'jsonc' }, completionProvider));
        subscriptions.push(commands.registerCommand('lldb.insertDebugConfig',
            (...args) => completionProvider.insertDebugConfig(args)));

        subscriptions.push(debug.registerDebugAdapterDescriptorFactory('lldb', this));

        subscriptions.push(commands.registerCommand('lldb.getCargoLaunchConfigs', (uri) => this.getCargoLaunchConfigs(uri)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', (config) => pickProcess(context, false, config)));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', (config) => pickProcess(context, true, config)));
        subscriptions.push(commands.registerCommand('lldb.attach', () => this.attach()));
        subscriptions.push(commands.registerCommand('lldb.viewMemory', () => this.viewMemory()));
        subscriptions.push(commands.registerCommand('lldb.symbols', () => pickSymbol(debug.activeDebugSession)));
        subscriptions.push(commands.registerCommand('lldb.alternateBackend', () => alternateBackend(this.context.extensionPath)));
        subscriptions.push(commands.registerCommand('lldb.selfTest', () => this.runSelfTest()));
        subscriptions.push(commands.registerCommand('lldb.commandPrompt', () => commandPrompt(this.context.extensionPath)));

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
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

        subscriptions.push(lm.registerTool('codelldb_session_info', new SessionInfoTool()));
        subscriptions.push(lm.registerTool('codelldb', new LLDBCommandTool()));
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
        install.ensurePlatformPackage(this.context.extensionPath, output, false);

        let context = this.context;
        context.environmentVariableCollection.description = 'No-config debugging';
        context.environmentVariableCollection.prepend('PATH', path.join(context.extensionPath, 'bin') + path.delimiter);
        if (context.storageUri?.fsPath) {
            if (!await async.fs.exists(context.storageUri.fsPath))
                await async.fs.mkdir(context.storageUri.fsPath);
            this.rpcFile = Uri.joinPath(context.storageUri, 'rpcaddress.txt').fsPath;
            context.environmentVariableCollection.replace('CODELLDB_LAUNCH_CONNECT_FILE', this.rpcFile);
        }

        this.updateRpcServer();
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

            if (this.rpcFile && await async.fs.exists(this.rpcFile)) {
                await async.fs.unlink(this.rpcFile);
            }
        }
        let config = getExtensionConfig();
        let options = config.get<any>('rpcServer');
        if (options) {
            output.appendLine(`Starting RPC server with: ${inspect(options)}`);
            this.rpcServer = new RpcLaunchServer({ token: options.token });
            await this.rpcServer.listen(options);

            let address = this.rpcServer.inner.address();
            if (this.rpcFile && address) {
                if (typeof (address) == 'object') {
                    let ainfo = address as AddressInfo;
                    address = `${ainfo.address}:${ainfo.port}`;
                }
                await async.fs.writeFile(this.rpcFile, address);

                let launch_config = `{ token: "${options.token}" }`;
                this.context.environmentVariableCollection.replace('CODELLDB_LAUNCH_CONFIG', launch_config);
            }
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

    // Discover debuggable targets in the current workspace and generate debug configs for them
    async discoverDebugConfigurations(
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        if (workspaceFolder) { // Need working directory for Cargo
            try {
                let cargo = new Cargo(workspaceFolder, cancellation);
                return await cargo.getLaunchConfigs();
            } catch (err: any) {
            }
        }
        return [];
    }


    // Called when:
    // 1. User creates launch.json (kind: Initial)
    // 2. User executes "Debug: Select and Start Debugging" command (kind: Dynamic)
    async provideDebugConfigurations(
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken,
        kind: DebugConfigurationProviderTriggerKind = DebugConfigurationProviderTriggerKind.Initial
    ): Promise<DebugConfiguration[]> {
        let configs = await this.discoverDebugConfigurations(workspaceFolder, cancellation);
        if (configs.length > 0)
            return configs;
        if (kind == DebugConfigurationProviderTriggerKind.Initial) {
            return [{
                name: 'Launch',
                type: 'lldb',
                request: 'launch',
                program: '${workspaceRoot}/<your program>',
                args: [],
                cwd: '${workspaceRoot}'
            }];
        } else {
            return [];
        }
    }

    // Called when debugging starts without a launch.json file
    async getLaunchLessConfig(
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration | undefined | null> {
        let configs = await this.discoverDebugConfigurations(workspaceFolder, cancellation);
        if (configs.length == 0)
            return null;
        if (configs.length == 1)
            return configs[0];
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
            if (!config)
                return config;
            debugConfig = config;
        }

        if (!await this.checkPrerequisites(folder))
            return undefined;

        let launchDefaults = getExtensionConfig(folder, 'launch');
        this.mergeWorkspaceSettings(debugConfig, launchDefaults);

        let dbgconfigConfig = getExtensionConfig(folder, 'dbgconfig');
        debugConfig = util.expandDbgConfig(debugConfig, dbgconfigConfig);

        // Convert legacy "request":"custom" to "request":"launch"
        if (debugConfig.request == 'custom') {
            debugConfig.request = 'launch';
        }

        if (typeof debugConfig.args == 'string') {
            debugConfig.args = stringArgv(debugConfig.args);
        }

        debugConfig.relativePathBase = debugConfig.relativePathBase || folder?.uri.fsPath || workspace.rootPath;
        debugConfig._adapterSettings = this.settingsManager.getAdapterSettings(folder);

        return debugConfig;
    }

    async resolveDebugConfigurationWithSubstitutedVariables(
        folder: WorkspaceFolder | undefined,
        debugConfig: DebugConfiguration,
        cancellation?: CancellationToken
    ): Promise<DebugConfiguration | undefined | null> {
        if (debugConfig.cargo) {
            let cargo = new Cargo(folder, cancellation);
            let launcher = path.join(this.context.extensionPath, 'bin', 'codelldb-launch');
            debugConfig = await cargo.resolveCargoConfig(debugConfig, launcher);
        }
        if (cancellation?.isCancellationRequested)
            return undefined;

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

        let startOptions = this.getAdapterStartOptions(session.workspaceFolder, adapterSettings);
        let connector = new ReverseAdapterConnector(startOptions.authToken as string);
        startOptions.port = await connector.listen();

        try {
            await this.startDebugAdapter(startOptions);
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
            await this.runSelfTest();
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
        mergeConfig('gracefulShutdown', true);
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

    async getCargoLaunchConfigs(resource?: Uri) {
        try {
            resource = resource ?? window.activeTextEditor?.document.uri!;
            let cargo = new Cargo(workspace.getWorkspaceFolder(resource));
            let configurations = await cargo.getLaunchConfigs(resource.fsPath);
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
            await showErrorWithLog(err.message);
        }
    }

    async startDebugAdapter(options: adapter.AdapterStartOptions): Promise<async.cp.ChildProcess> {
        output.appendLine('Launching adapter');
        output.appendLine(`liblldb: ${options.liblldb}`);
        output.appendLine(`lldbServer: ${options.lldbServer}`);
        output.appendLine(`environment: ${inspect(options.extraEnv)}`);
        output.appendLine(`settings: ${inspect(options.adapterSettings)}`);

        let adapterProcess = await adapter.start(options);
        util.logProcessOutput(adapterProcess, output);

        adapterProcess.on('exit', async (code, signal) => {
            output.appendLine(`Debug adapter exit code=${code}, signal=${signal}.`);
            if (code != 0) {
                showErrorWithLog('Oops!  The debug adapter has terminated abnormally.');
            }
        });
        return adapterProcess;
    }

    getAdapterStartOptions(
        folder: WorkspaceFolder | undefined,
        adapterSettings: AdapterSettings = {},
        port: number = 0
    ): adapter.AdapterStartOptions {
        let config = getExtensionConfig(folder);
        let verboseLogging = config.get<boolean>('verboseLogging', false);
        let liblldb = config.get<string>('library');
        let adapterEnv = Object.assign({}, config.get<object>('adapterEnv')) as Dict<string>;
        let lldbServer = config.get<string>('server');
        if (config.get<boolean>('useNativePDBReader'))
            adapterEnv['LLDB_USE_NATIVE_PDB_READER'] = 'true';
        let authToken = crypto.randomBytes(16).toString('base64');
        return {
            extensionPath: this.context.extensionPath,
            liblldb: liblldb,
            lldbServer: lldbServer,
            extraEnv: adapterEnv,
            workDir: workspace.rootPath,
            port: port,
            connect: true,
            authToken: authToken,
            adapterSettings: adapterSettings,
            verboseLogging: verboseLogging
        }
    }

    async checkPrerequisites(folder?: WorkspaceFolder): Promise<boolean> {
        if (!await install.ensurePlatformPackage(this.context.extensionPath, output, true))
            return false;
        return true;
    }

    async runSelfTest(folder?: WorkspaceFolder) {
        let startOptions = this.getAdapterStartOptions(folder);
        let succeeded = await selfTest(startOptions);
        if (succeeded) {
            window.showInformationMessage('CodeLLDB self-test completed successfuly.', { modal: true });
        } else {
            output.show();
            window.showErrorMessage('CodeLLDB self-test has failed.  Please check log output.', { modal: true });
        }
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
