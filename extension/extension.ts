'use strict';
import {
    workspace, languages, window, commands, debug,
    ExtensionContext, Disposable, QuickPickItem, Uri, Event, EventEmitter,
    DebugConfiguration, DebugSession
} from 'vscode';
import { DebugProtocol } from 'vscode-debugprotocol';
import * as path from 'path';
import * as startup from './startup';
import * as util from './util';

export function activate(context: ExtensionContext) {
    new Extension(context);
}

interface ProvideContentResponse extends DebugProtocol.Response {
    body: {
        content: string;
    }
}

class Extension {
    context: ExtensionContext;
    launching: [string, startup.AdapterProcess][] = [];
    activeSessions: { [key: string]: startup.AdapterProcess; } = {};
    previewContent: { [key: string]: string; } = {};
    previewContentChanged: EventEmitter<Uri> = new EventEmitter<Uri>();

    constructor(context: ExtensionContext) {
        this.context = context;
        let subscriptions = context.subscriptions;

        subscriptions.push(commands.registerCommand('lldb.getAdapterExecutable',
            () => startup.getAdapterExecutable(this.context)));
        subscriptions.push(commands.registerCommand('lldb.startDebugSession',
            (args) => this.startDebugSession(args)));
        subscriptions.push(commands.registerCommand('lldb.showDisassembly',
            () => this.showDisassembly()));
        subscriptions.push(commands.registerCommand('lldb.toggleDisassembly',
            () => this.toggleDisassembly()));
        subscriptions.push(commands.registerCommand('lldb.displayFormat',
            () => this.displayFormat()));
        subscriptions.push(commands.registerCommand('lldb.launchDebugServer',
            () => startup.launchDebugServer(this.context)));
        subscriptions.push(commands.registerCommand('lldb.diagnose',
            () => startup.diagnose()));
        subscriptions.push(commands.registerCommand('lldb.pickProcess',
            () => this.pickProcess(false)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess',
            () => this.pickProcess(true)));

        let extension = this;
        subscriptions.push(workspace.registerTextDocumentContentProvider('debugger', {
            get onDidChange(): Event<Uri> {
                return extension.previewContentChanged.event;
            },
            async provideTextDocumentContent(uri): Promise<string> {
                return extension.provideHtmlContent(uri);
            }
        }));

        subscriptions.push(debug.onDidStartDebugSession(session => {
            if (session.type == 'lldb') {
                // VSCode does not provide a way to associate a piece of data with a DebugSession
                // being launched via vscode.startDebug, so we are saving AdapterProcess'es in
                // this.launching and then try to re-associate them with correct DebugSessions
                // once we get this notification. >:-(
                for (var i = 0; i < this.launching.length; ++i) {
                    let [name, adapter] = this.launching[i];
                    if (session.name == name) {
                        this.activeSessions[session.id] = adapter;
                        this.launching.splice(i, 1);
                        return;
                    }
                    // Clean out entries that became stale for some reason.
                    if (!adapter.isAlive) {
                        this.launching.splice(i--, 1);
                    }
                }
            }
        }, this));
        subscriptions.push(debug.onDidTerminateDebugSession(session => {
            if (session.type == 'lldb') {
                let adapter = this.activeSessions[session.id];
                if (adapter) {
                    // Adapter should exit automatically when VSCode disconnects, but in case it
                    // doesn't, we kill it (after giving a bit of time to shut down gracefully).
                    setTimeout(adapter.terminate, 1500);
                }
                delete this.activeSessions[session.id];
            }
        }, this));
        subscriptions.push(debug.onDidReceiveDebugSessionCustomEvent(e => {
            if (e.session.type == 'lldb') {
                if (e.event = 'displayHtml') {
                    this.onDisplayHtml(e.body);
                }
            }
        }, this));
    }

    // Invoked by VSCode to initiate a new debugging session.
    async startDebugSession(config: DebugConfiguration) {
        if (!this.context.globalState.get('lldb_works')) {
            window.showInformationMessage("Since this is the first time you are starting LLDB, I'm going to run some quick diagnostics...");
            let succeeded = await startup.diagnose();
            this.context.globalState.update('lldb_works', succeeded);
            if (!succeeded)
                return;
        }
        try {
            if (!config.debugServer) {
                let adapter = await startup.startDebugAdapter(this.context);
                this.launching.push([config.name, adapter]);
                config.debugServer = adapter.port;
            }
            await commands.executeCommand('vscode.startDebug', config);
        } catch (err) {
            startup.analyzeStartupError(err);
        }
    }

    async showDisassembly() {
        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            let selection = await window.showQuickPick(['always', 'auto', 'never']);
            debug.activeDebugSession.customRequest('showDisassembly', { value: selection });
        }
    }

    async  toggleDisassembly() {
        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            debug.activeDebugSession.customRequest('showDisassembly', { value: 'toggle' });
        }
    }

    async  displayFormat() {
        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            let selection = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
            debug.activeDebugSession.customRequest('displayFormat', { value: selection });
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

    async onDisplayHtml(body: any) {
        this.previewContent = body.content;
        for (var keyUri in body.content) {
            this.previewContentChanged.fire(Uri.parse(keyUri));
        }
        await commands.executeCommand('vscode.previewHtml',
            body.uri, body.position, body.title, { allowScripts: true, allowSvgs: true });
    }

    async provideHtmlContent(uri: Uri): Promise<string> {
        let uriString = uri.toString();
        if (this.previewContent.hasOwnProperty(uriString)) {
            return this.previewContent[uriString];
        }
        let result = await commands.executeCommand<ProvideContentResponse>(
            'workbench.customDebugRequest', 'provideContent', { uri: uriString });
        if (result === undefined) {
            return "Not available";
        } else {
            return result.body.content;
        }
    }
};
