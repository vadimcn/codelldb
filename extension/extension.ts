import {
    workspace, languages, window, commands, debug,
    ExtensionContext, Disposable, QuickPickItem, Uri, Event, EventEmitter,
    TextDocumentContentProvider, WorkspaceConfiguration, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration, DebugSession, DebugSessionCustomEvent
} from 'vscode';
import { DebugProtocol } from 'vscode-debugprotocol';
import * as path from 'path';
import * as cp from 'child_process';
import { format, inspect } from 'util';
import * as startup from './startup';
import * as util from './util';
import * as cargo from './cargo';

export let output = window.createOutputChannel('LLDB');

export interface Dict<T> {
    [key: string]: T;
}

class ActiveDebugSession {
    constructor(adapter: startup.AdapterProcess, debugSession: DebugSession) {
        this.adapter = adapter;
        this.debugSession = debugSession;
    }
    adapter: startup.AdapterProcess;
    debugSession: DebugSession;
    previewContent: Dict<string> = {};
}

class DisplaySettings {
    showDisassembly: string = 'auto'; // 'always' | 'auto' | 'never'
    displayFormat: string = 'auto'; // 'auto' | 'hex' | 'decimal' | 'binary'
    dereferencePointers: boolean = true;
    containerSummary: boolean = true;
};

export function activate(context: ExtensionContext) {
    new Extension(context);
}

class Extension implements TextDocumentContentProvider, DebugConfigurationProvider {
    context: ExtensionContext;
    launching: [string, startup.AdapterProcess][] = [];
    activeSessions: Dict<ActiveDebugSession> = {};
    previewContentChanged: EventEmitter<Uri> = new EventEmitter<Uri>();

    constructor(context: ExtensionContext) {
        this.context = context;
        let subscriptions = context.subscriptions;

        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));
        subscriptions.push(debug.onDidStartDebugSession(this.onStartedDebugSession, this));
        subscriptions.push(debug.onDidTerminateDebugSession(this.onTerminatedDebugSession, this));
        subscriptions.push(debug.onDidReceiveDebugSessionCustomEvent(this.onDebugSessionCustomEvent, this));
        subscriptions.push(workspace.registerTextDocumentContentProvider('debugger', this));

        subscriptions.push(commands.registerCommand('lldb.diagnose', startup.diagnose));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', () => this.pickProcess(false)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', () => this.pickProcess(true)));
        subscriptions.push(commands.registerCommand('lldb.launchDebugServer', () => startup.launchDebugServer(this.context)));

        this.registerDisplaySettingCommand('lldb.showDisassembly', async (settings) => {
            settings.showDisassembly = await window.showQuickPick(['always', 'auto', 'never']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDisassembly', async (settings) => {
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
        });
        this.registerDisplaySettingCommand('lldb.displayFormat', async (settings) => {
            settings.displayFormat = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDerefPointers', async (settings) => {
            settings.dereferencePointers = !settings.dereferencePointers;
        });
        this.registerDisplaySettingCommand('lldb.toggleContainerSummary', async (settings) => {
            settings.containerSummary = !settings.containerSummary;
        });
    }

    async provideDebugConfigurations(
        folder: WorkspaceFolder | undefined,
        token?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        let debugConfigs = await cargo.getLaunchConfigs(folder ? folder.uri.fsPath : workspace.rootPath);
        if (debugConfigs.length == 0) {
            debugConfigs.push({
                type: 'lldb',
                request: 'launch',
                name: 'Debug',
                program: '${workspaceFolder}/<your program>',
                args: [],
                cwd: '${workspaceFolder}'
            });
        }
        return debugConfigs;
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        launchConfig: DebugConfiguration,
        token?: CancellationToken
    ): Promise<DebugConfiguration> {
        if (!this.context.globalState.get('lldb_works')) {
            window.showInformationMessage("Since this is the first time you are starting LLDB, I'm going to run some quick diagnostics...");
            let succeeded = await startup.diagnose();
            this.context.globalState.update('lldb_works', succeeded);
            if (!succeeded) {
                return null;
            }
        }

        output.clear();

        let workspaceConfig = workspace.getConfiguration('lldb.launch', folder ? folder.uri : undefined);
        launchConfig = this.mergeWorkspaceSettings(launchConfig, workspaceConfig);

        let dbgconfigConfig = workspace.getConfiguration('lldb.dbgconfig', folder ? folder.uri : undefined);
        launchConfig = this.expandDbgConfig(launchConfig, dbgconfigConfig);

        // Transform "request":"custom" to "request":"launch" + "custom":true
        if (launchConfig.request == 'custom') {
            launchConfig.request = 'launch';
            launchConfig.custom = true;
        }

        // Deal with Cargo
        let cargoDict: Dict<string> = {};
        if (launchConfig.cargo != undefined) {
            let cargoCwd = folder ? folder.uri.fsPath : workspace.rootPath;
            cargoDict.program = await cargo.getProgramFromCargo(launchConfig.cargo, cargoCwd);
            delete launchConfig.cargo;

            // Expand ${cargo:program}.
            launchConfig = cargo.expandCargo(launchConfig, cargoDict);

            if (launchConfig.program == undefined) {
                launchConfig.program = cargoDict.program;
            }

            // Add 'rust' to sourceLanguages, since this project obviously (ha!) involves Rust.
            if (!launchConfig.sourceLanguages)
                launchConfig.sourceLanguages = [];
            launchConfig.sourceLanguages.push('rust');
        }

        let adapterParams: any = {};
        if (launchConfig.sourceLanguages) {
            adapterParams.sourceLanguages = launchConfig.sourceLanguages;
            delete launchConfig.sourceLanguages;
        }

        output.appendLine('Starting new session with:');
        output.appendLine(inspect(launchConfig));

        try {
            // If configuration does not provide debugServer explicitly, launch new adapter.
            if (!launchConfig.debugServer) {
                let adapter = await startup.startDebugAdapter(this.context, folder, adapterParams);
                this.launching.push([launchConfig.name, adapter]);
                launchConfig.debugServer = adapter.port;
            }
            // For adapter debugging
            if (launchConfig._adapterStartDelay) {
                await new Promise(resolve => setTimeout(resolve, launchConfig._adapterStartDelay));
            }
            launchConfig._displaySettings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
            return launchConfig;
        } catch (err) {
            startup.analyzeStartupError(err);
            return null;
        }
    }

    // Merge launch configuration with workspace settings
    mergeWorkspaceSettings(debugConfig: DebugConfiguration, launchConfig: WorkspaceConfiguration): DebugConfiguration {
        let mergeConfig = (key: string, reverse: boolean = false) => {
            let value1 = util.getConfigNoDefault(launchConfig, key);
            let value2 = debugConfig[key];
            let value = !reverse ? util.mergeValues(value1, value2) : util.mergeValues(value2, value1);
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
        mergeConfig('sourceLanguages');
        mergeConfig('debugServer');
        return debugConfig;
    }

    // Expands variable references of the form ${dbgconfig:name} in all properties of launch configuration.
    expandDbgConfig(debugConfig: DebugConfiguration, dbgconfigConfig: WorkspaceConfiguration): DebugConfiguration {
        let dbgconfig: Dict<any> = Object.assign({}, dbgconfigConfig);

        // Compute fixed-point of expansion of dbgconfig properties.
        var expanding = '';
        var converged = true;
        let expander = (type: string, key: string) => {
            if (type == 'dbgconfig') {
                if (key == expanding)
                    throw new Error('Circular dependency detected during expansion of dbgconfig:' + key);
                let value = dbgconfig[key];
                if (value == undefined)
                    throw new Error('dbgconfig:' + key + ' is not defined');
                converged = false;
                return value.toString();
            }
            return null;
        };
        do {
            converged = true;
            for (var prop of Object.keys(dbgconfig)) {
                expanding = prop;
                dbgconfig[prop] = util.expandVariablesInObject(dbgconfig[prop], expander);
            }
        } while (!converged);

        // Now expand dbgconfigs in the launch configuration.
        debugConfig = util.expandVariablesInObject(debugConfig, (type, key) => {
            if (type == 'dbgconfig') {
                let value = dbgconfig[key];
                if (value == undefined)
                    throw new Error('dbgconfig:' + key + ' is not defined');
                return value.toString();
            }
            return null;
        });
        return debugConfig;
    }

    onStartedDebugSession(session: DebugSession) {
        if (session.type == 'lldb') {
            // VSCode does not provide a way to associate a piece of data with a DebugSession
            // being launched via vscode.startDebug, so we are saving AdapterProcess'es in
            // this.launching and then try to re-associate them with correct DebugSessions
            // once we get this notification. >:-(
            for (var i = 0; i < this.launching.length; ++i) {
                let [name, adapter] = this.launching[i];
                if (session.name == name) {
                    this.activeSessions[session.id] = new ActiveDebugSession(adapter, session);
                    this.launching.splice(i, 1);
                    return;
                }
                // Clean out entries that became stale for some reason.
                if (!adapter.isAlive) {
                    this.launching.splice(i--, 1);
                }
            }
        }
    }

    onTerminatedDebugSession(session: DebugSession) {
        if (session.type == 'lldb') {
            let activeSession = this.activeSessions[session.id];
            if (activeSession) {
                // Adapter should exit automatically when VSCode disconnects, but in case it
                // doesn't, we kill it (after giving a bit of time to shut down gracefully).
                setTimeout(() => activeSession.adapter.terminate(), 1500);
            }
            delete this.activeSessions[session.id];
        }
    }

    onDebugSessionCustomEvent(e: DebugSessionCustomEvent) {
        if (e.session.type == 'lldb') {
            if (e.event = 'displayHtml') {
                this.onDisplayHtml(e.session.id, e.body);
            }
        }
    }

    registerDisplaySettingCommand(command: string, updater: (settings: DisplaySettings) => Promise<void>) {
        this.context.subscriptions.push(commands.registerCommand(command, async () => {
            if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
                var settings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
                await updater(settings);
                this.context.globalState.update('display_settings', settings);
                await debug.activeDebugSession.customRequest('displaySettings', settings);
            }
        }));
    }

    async pickProcess(currentUserOnly: boolean): Promise<string> {
        let items = util.getProcessList(currentUserOnly);
        let item = await window.showQuickPick(items);
        if (item) {
            return item.pid.toString();
        } else {
            throw Error('Cancelled');
        }
    }

    /// HTML display stuff ///

    normalizeUri(uri: Uri, sessionId: string): Uri {
        if (uri.scheme && uri.scheme != 'debugger')
            return uri; // Pass through non-debugger URIs.
        return uri.with({ scheme: 'debugger', authority: sessionId });
    }

    async onDisplayHtml(sessionId: string, body: any) {
        var documentUri = this.normalizeUri(Uri.parse(body.uri), sessionId);
        for (var key in body.content) {
            var contentUri = this.normalizeUri(Uri.parse(key), sessionId);
            let content = body.content[key];
            if (content != null) {
                this.activeSessions[sessionId].previewContent[contentUri.toString()] = content;
            } else {
                delete this.activeSessions[sessionId].previewContent[contentUri.toString()];
            }
            if (contentUri.toString() != documentUri.toString()) {
                this.previewContentChanged.fire(contentUri);
            }
        }
        this.previewContentChanged.fire(documentUri);
        await commands.executeCommand('vscode.previewHtml', documentUri.toString(),
            body.position, body.title, { allowScripts: true, allowSvgs: true });
    }

    async provideTextDocumentContent(uri: Uri): Promise<string> {
        if (uri.scheme != 'debugger')
            return null; // Should not happen, as we've only registered for 'debugger'.

        let activeSession = this.activeSessions[uri.authority];
        if (!activeSession) {
            console.error('provideTextDocumentContent: Did not find an active debug session for %s', uri.toString());
            return null;
        }

        let uriString = uri.toString();
        if (activeSession.previewContent.hasOwnProperty(uriString)) {
            return activeSession.previewContent[uriString];
        }
        let result = await activeSession.debugSession.customRequest('provideContent', { uri: uriString });
        return result.content;
    }

    get onDidChange(): Event<Uri> {
        return this.previewContentChanged.event;
    }
};
