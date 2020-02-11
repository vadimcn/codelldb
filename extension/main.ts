import {
    workspace, window, commands, debug, env,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration, DebugAdapterDescriptorFactory, DebugSession, DebugAdapterExecutable,
    DebugAdapterDescriptor, DebugAdapterServer, extensions, Uri, StatusBarAlignment, QuickPickItem, StatusBarItem, UriHandler, ConfigurationTarget,
} from 'vscode';
import { inspect } from 'util';
import { ChildProcess } from 'child_process';
import * as path from 'path';
import * as querystring from 'querystring';
import JSON5 from 'json5';
import stringArgv from 'string-argv';
import * as diagnostics from './diagnostics';
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


export let output = window.createOutputChannel('LLDB');

// Main entry point
export function activate(context: ExtensionContext) {
    let extension = new Extension(context);
    extension.onActivate();
}

class Extension implements DebugConfigurationProvider, DebugAdapterDescriptorFactory, UriHandler {
    context: ExtensionContext;
    htmlViewer: htmlView.DebuggerHtmlView;
    status: StatusBarItem;
    loadedModules: ModuleTreeDataProvider;

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

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.displayFormat') ||
                event.affectsConfiguration('lldb.showDisassembly') ||
                event.affectsConfiguration('lldb.dereferencePointers') ||
                event.affectsConfiguration('lldb.suppressMissingSourceFiles') ||
                event.affectsConfiguration('lldb.evaluationTimeout') ||
                event.affectsConfiguration('lldb.consoleMode')) {
                this.propagateDisplaySettings();
            }
            if (event.affectsConfiguration('lldb.library') ||
                event.affectsConfiguration('lldb.libpython')) {
                this.adapterDylibsCache = null;
            }
        }));

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
    }

    async handleUri(uri: Uri) {
        try {
            output.appendLine(`Handling uri: ${uri}`);
            let query = decodeURIComponent(uri.query);
            output.appendLine(`Decoded query: ${query}`);

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
                Object.assign(debugConfig, JSON5.parse(query));
                debugConfig.name = debugConfig.name || debugConfig.program;
                await debug.startDebugging(undefined, debugConfig);

            } else {
                throw new Error(`Unsupported Uri path: ${uri.path}`);
            }
        } catch (err) {
            await window.showErrorMessage(err.message);
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
    getAdapterSettings(): AdapterSettings {
        let folder = debug.activeDebugSession?.workspaceFolder;
        let config = this.getExtensionConfig(folder);
        let settings: AdapterSettings = {
            displayFormat: config.get('displayFormat'),
            showDisassembly: config.get('showDisassembly'),
            dereferencePointers: config.get('dereferencePointers'),
            suppressMissingSourceFiles: config.get('suppressMissingSourceFiles'),
            evaluationTimeout: config.get('evaluationTimeout'),
            consoleMode: config.get('consoleMode'),
            sourceLanguages: null
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
    }

    // This is called When configuration change is detected. Updates UI, and if a debug session
    // is active, pushes updated settings to the adapter as well.
    async propagateDisplaySettings() {
        let settings = this.getAdapterSettings();

        this.status.text =
            `Format: ${settings.displayFormat}  ` +
            `Disasm: ${settings.showDisassembly}  ` +
            `Deref: ${settings.dereferencePointers ? 'on' : 'off'}`;

        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            await debug.activeDebugSession.customRequest('adapterSettings', settings);
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

    async onActivate() {
        let pkg = extensions.getExtension('vadimcn.vscode-lldb').packageJSON;
        let currVersion = pkg.version;
        let lastVersion = this.context.globalState.get('lastLaunchedVersion');
        if (lastVersion != undefined && currVersion != lastVersion) {
            this.context.globalState.update('lastLaunchedVersion', currVersion);
            let choice = await window.showInformationMessage('CodeLLDB extension has been updated', 'What\'s new?');
            if (choice != null) {
                let changelog = path.join(this.context.extensionPath, 'CHANGELOG.md')
                let uri = Uri.file(changelog);
                await commands.executeCommand('markdown.showPreview', uri, null, { locked: true });
            }
        }
        this.propagateDisplaySettings();
        install.ensurePlatformPackage(this.context, output, false);
    }

    async provideDebugConfigurations(
        workspaceFolder: WorkspaceFolder | undefined,
        token?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        try {
            let config = this.getExtensionConfig(workspaceFolder);
            let folder = workspaceFolder ? workspaceFolder.uri.fsPath : workspace.rootPath;
            let cargo = new Cargo(folder, config.get('adapterEnv', {}));
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
            program: '${workspaceFolder}/<your program>',
            args: [],
            cwd: '${workspaceFolder}'
        }];
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        launchConfig: DebugConfiguration,
        token?: CancellationToken
    ): Promise<DebugConfiguration> {
        output.clear();

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
            let cargoTomlFolder = folder ? folder.uri.fsPath : workspace.rootPath;
            let cargo = new Cargo(cargoTomlFolder, config.get('adapterEnv', {}));
            let cargoDict = { program: await cargo.getProgramFromCargoConfig(launchConfig.cargo) };
            delete launchConfig.cargo;

            // Expand ${cargo:program}.
            launchConfig = expandCargo(launchConfig, cargoDict);

            if (launchConfig.program == undefined) {
                launchConfig.program = cargoDict.program;
            }

            // Add 'rust' to sourceLanguages, since this project obviously (ha!) involves Rust.
            if (!launchConfig.sourceLanguages)
                launchConfig.sourceLanguages = [];
            launchConfig.sourceLanguages.push('rust');
        }
        output.appendLine(`configuration: ${inspect(launchConfig)}`);
        launchConfig._adapterSettings = this.getAdapterSettings();
        return launchConfig;
    }

    async createDebugAdapterDescriptor(session: DebugSession, executable: DebugAdapterExecutable | undefined): Promise<DebugAdapterDescriptor> {
        let lldbConfig = this.getExtensionConfig(session.workspaceFolder);
        let adapterParams: any = this.getAdapterParameters(lldbConfig);
        if (session.configuration.sourceLanguages) {
            adapterParams.sourceLanguages = session.configuration.sourceLanguages;
            delete session.configuration.sourceLanguages;
        }

        try {
            let [adapter, port] = await this.startDebugAdapter(session.workspaceFolder, adapterParams);
            let descriptor = new DebugAdapterServer(port);
            return descriptor;
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

    getAdapterParameters(config: WorkspaceConfiguration, params: Dict<any> = {}): Dict<any> {
        util.setIfDefined(params, config, 'reverseDebugging');
        util.setIfDefined(params, config, 'suppressMissingSourceFiles');
        util.setIfDefined(params, config, 'evaluationTimeout');
        util.setIfDefined(params, config, 'consoleMode');
        return params;
    }

    async getCargoLaunchConfigs() {
        try {
            let config = this.getExtensionConfig();
            let cargo = new Cargo(workspace.rootPath, config.get('adapterEnv'));
            let debugConfigs = await cargo.getLaunchConfigs();
            let doc = await workspace.openTextDocument({
                content: JSON.stringify(debugConfigs, null, 4),
                language: 'jsonc'
            });
            await window.showTextDocument(doc, 1, false);
        } catch (err) {
            output.show();
            window.showErrorMessage(err.toString());
        }
    }

    async startDebugAdapter(
        folder: WorkspaceFolder | undefined,
        adapterParams: Dict<string>
    ): Promise<[ChildProcess, number]> {
        let config = this.getExtensionConfig(folder);
        let adapterEnv = config.get('adapterEnv', {});
        let verboseLogging = config.get<boolean>('verboseLogging');
        let [liblldb, libpython] = await this.getAdapterDylibs(config);

        if (verboseLogging) {
            output.appendLine(`liblldb: ${liblldb}`);
            output.appendLine(`libpython: ${libpython}`);
            output.appendLine(`environment: ${inspect(adapterEnv)}`);
            output.appendLine(`params: ${inspect(adapterParams)}`);
        }

        let adapterProcess = await adapter.start(liblldb, libpython, {
            extensionRoot: this.context.extensionPath,
            extraEnv: adapterEnv,
            workDir: workspace.rootPath,
            adapterParameters: adapterParams,
            verboseLogging: verboseLogging
        });

        util.logProcessOutput(adapterProcess, output);
        let port = await adapter.getDebugServerPort(adapterProcess);

        adapterProcess.on('exit', async (code, signal) => {
            output.appendLine(`Debug adapter exit code=${code}, signal=${signal}.`);
            if (code != 0) {
                let result = await window.showErrorMessage('Oops!  The debug adapter has terminated abnormally.', 'Open log');
                if (result != undefined) {
                    output.show();
                }
            }
        });
        return [adapterProcess, port];
    }

    // Resolve paths of the native adapter libraries and cache them.
    async getAdapterDylibs(config: WorkspaceConfiguration): Promise<[string, string]> {
        if (!this.adapterDylibsCache) {
            let libpython;
            let liblldb = util.getConfigNoDefault(config, 'library');
            if (liblldb) {
                liblldb = await adapter.findLibLLDB(liblldb)
                // Don't preload libpython, because external backend will have been linked to a specific Python version.
                libpython = null;
            } else {
                liblldb = await adapter.findLibLLDB(path.join(this.context.extensionPath, 'lldb'));
                // Bundled liblldb is weak-linked, so we need to locate some version of Python 3.x.
                libpython = util.getConfigNoDefault(config, 'libpython');
                if (!libpython) {
                    libpython = await adapter.findLibPython(this.context.extensionPath, config.get('adapterEnv'));
                }
            }
            this.adapterDylibsCache = [liblldb, libpython];
        }
        return this.adapterDylibsCache;
    }
    adapterDylibsCache: [string, string] = null;

    async checkPrerequisites(folder?: WorkspaceFolder): Promise<boolean> {
        if (!await this.checkPython(folder))
            return false;
        if (!await install.ensurePlatformPackage(this.context, output, true))
            return false;
        return true;
    }

    async runDiagnostics(folder?: WorkspaceFolder) {
        let succeeded;
        try {
            succeeded = await this.checkPython(folder);
            if (succeeded) {
                let [_, port] = await this.startDebugAdapter(folder, {});
                await diagnostics.testAdapter(port);
            }
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

    async checkPython(folder?: WorkspaceFolder): Promise<boolean> {
        if (process.platform == 'win32') {
            // On Windows libpython is required.
            let config = this.getExtensionConfig(folder);
            let [liblldb, libpython] = await this.getAdapterDylibs(config);
            if (!libpython) {
                let action = await window.showErrorMessage(
                    `CodeLLDB requires Python 3.3 or later (64-bit), but looks like it is not installed on this machine.`,
                    { modal: true },
                    'Take me to Python website');
                if (action != null) {
                    env.openExternal(Uri.parse('https://www.python.org/downloads/windows/'));
                }
                return false;
            }
        }
        return true;
    }

    async attach() {
        let debugConfig: DebugConfiguration = {
            type: 'lldb',
            request: 'attach',
            name: '',
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

    getExtensionConfig(folder?: WorkspaceFolder, key: string = 'lldb'): WorkspaceConfiguration {
        return workspace.getConfiguration(key, folder?.uri);
    }
}


