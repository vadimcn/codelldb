'use strict';
import {
    workspace, languages, window, commands, debug,
    ExtensionContext, Disposable, QuickPickItem, Uri, Event, EventEmitter,
    TextDocumentContentProvider, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration, DebugSession, DebugSessionCustomEvent
} from 'vscode';
import { DebugProtocol } from 'vscode-debugprotocol';
import * as path from 'path';
import * as startup from './startup';
import { format, inspect } from 'util';
import * as util from './util';

let output = window.createOutputChannel('LLDB');
(<any>startup).output = output;

export function activate(context: ExtensionContext) {
    new Extension(context);
}

interface Dict<T> {
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
};

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
        subscriptions.push(commands.registerCommand('lldb.showDisassembly', this.showDisassembly, this));
        subscriptions.push(commands.registerCommand('lldb.toggleDisassembly', this.toggleDisassembly, this));
        subscriptions.push(commands.registerCommand('lldb.displayFormat', this.displayFormat, this));
        subscriptions.push(commands.registerCommand('lldb.toggleDerefPointers', this.toggleDerefPointers, this));
        subscriptions.push(commands.registerCommand('lldb.diagnose', startup.diagnose));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', () => this.pickProcess(false)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', () => this.pickProcess(true)));
        subscriptions.push(commands.registerCommand('lldb.launchDebugServer', () => startup.launchDebugServer(this.context)));
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        config: DebugConfiguration,
        token?: CancellationToken): Promise<DebugConfiguration> {

        if (!this.context.globalState.get('lldb_works')) {
            window.showInformationMessage("Since this is the first time you are starting LLDB, I'm going to run some quick diagnostics...");
            let succeeded = await startup.diagnose();
            this.context.globalState.update('lldb_works', succeeded);
            if (!succeeded) {
                return null;
            }
        }
        try {
            let launch = workspace.getConfiguration('lldb.launch');
            // Merge launch config with workspace settings
            let mergeConfig = (key: string, reverse: boolean = false) => {
                let value1 = util.getConfigNoDefault(launch, key);
                let value2 = config[key];
                let value = !reverse ? util.mergeValues(value1, value2) : util.mergeValues(value2, value1);
                if (!util.isEmpty(value))
                    config[key] = value;
            }
            mergeConfig('initCommands');
            mergeConfig('preRunCommands');
            mergeConfig('exitCommands', true);
            mergeConfig('env');
            mergeConfig('cwd');
            mergeConfig('terminal');
            mergeConfig('stdio');
            mergeConfig('sourceMap');
            mergeConfig('sourceLanguages');

            output.clear();
            output.appendLine('Starting new session with:');
            output.appendLine(inspect(config));

            // If configuration does not provide debugServer explicitly, launch new adapter.
            if (!config.debugServer) {
                let adapter = await startup.startDebugAdapter(this.context);
                this.launching.push([config.name, adapter]);
                config.debugServer = adapter.port;
            }
            // For adapter debugging
            if (config._adapterStartDelay) {
                await new Promise(resolve => setTimeout(resolve, config._adapterStartDelay));
            }
            config._displaySettings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
            return config;
        } catch (err) {
            startup.analyzeStartupError(err);
            return null;
        }
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

    async showDisassembly() {
        let selection = await window.showQuickPick(['always', 'auto', 'never']);
        this.updateDisplaySettings(settings => settings.showDisassembly = selection);
    }

    async toggleDisassembly() {
        await this.updateDisplaySettings(settings => {
            if (settings.showDisassembly == 'auto') {
                settings.showDisassembly = 'always';
            } else {
                settings.showDisassembly = 'auto';
        }
        });
    }

    async displayFormat() {
        let selection = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        await this.updateDisplaySettings(settings => settings.displayFormat = selection);
    }

    async toggleDerefPointers() {
        await this.updateDisplaySettings(
            settings => settings.dereferencePointers = !settings.dereferencePointers);
    }

    async updateDisplaySettings(updater: (settings: DisplaySettings) => void) {
        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            var settings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
            updater(settings);
            this.context.globalState.update('display_settings', settings);
            await debug.activeDebugSession.customRequest('displaySettings', settings);
        }
    }

    async pickProcess(currentUserOnly: boolean): Promise<number> {
        let items = util.getProcessList(currentUserOnly);
        let item = await window.showQuickPick(items);
        if (item) {
            return item.pid;
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
