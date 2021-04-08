import {
    debug, env, commands, TreeDataProvider, TreeItem, ProviderResult, EventEmitter,
    ExtensionContext, TreeItemCollapsibleState, DebugSession, DebugAdapterTracker
} from 'vscode';
import { DebugProtocol } from 'vscode-debugprotocol';
import { Dict } from './novsc/commonTypes';

type Module = DebugProtocol.Module

class ModuleProperty {
    key: string;
    value: string;

    constructor(key: string, value: string) {
        this.key = key;
        this.value = value;
    }
}

type Element = Module | ModuleProperty;


export class ModuleTreeDataProvider implements TreeDataProvider<Element> {

    sessions: Dict<Module[]> = {};
    activeSessionId: string = undefined;

    onDidChangeTreeDataEmitter = new EventEmitter<any>();
    readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

    constructor(context: ExtensionContext) {
        context.subscriptions.push(debug.registerDebugAdapterTrackerFactory('lldb', this));
        context.subscriptions.push(debug.onDidChangeActiveDebugSession(this.onChangedActiveDebugSession, this));
        context.subscriptions.push(commands.registerCommand('lldb.modules.copyValue', (arg) => this.copyValue(arg)));
    }

    modulesForSession(sessionId: string): Module[] {
        let modules = this.sessions[sessionId];
        if (modules == undefined) {
            modules = []
            this.sessions[sessionId] = modules;
        }
        return modules;
    }

    copyValue(prop: ModuleProperty): Thenable<void> {
        return env.clipboard.writeText(prop.value);
    }

    getChildren(element?: Element): ProviderResult<Element[]> {
        if (element == undefined) {
            return this.sessions[this.activeSessionId];
        } else if (element instanceof ModuleProperty) {
            return [];
        } else {
            let module = <Module>element;
            let props = [];
            if (module.path)
                props.push(new ModuleProperty('path', module.path));
            if (module.version)
                props.push(new ModuleProperty('version', module.version));
            if (module.symbolStatus)
                props.push(new ModuleProperty('symbols', module.symbolStatus));
            if (module.symbolFilePath)
                props.push(new ModuleProperty('symbol file path', module.symbolFilePath));
            if (module.addressRange)
                props.push(new ModuleProperty('load address', module.addressRange));
            return props;
        }
    }

    getTreeItem(element: Element): TreeItem {
        if (element instanceof ModuleProperty) {
            let item = new TreeItem(`${element.key}: ${element.value}`);
            item.contextValue = 'lldb.moduleProperty';
            return item;
        } else {
            let module = <Module>element;
            let item = new TreeItem(module.name, TreeItemCollapsibleState.Collapsed);
            return item;
        }
    }

    createDebugAdapterTracker(session: DebugSession): ProviderResult<DebugAdapterTracker> {
        return new AdapterTracker(session, this);
    }

    onChangedActiveDebugSession(session: DebugSession) {
        this.activeSessionId = session.id;
        this.onDidChangeTreeDataEmitter.fire(null);
    }
}

class AdapterTracker implements DebugAdapterTracker {
    session: DebugSession;
    treeView: ModuleTreeDataProvider;

    constructor(session: DebugSession, treeView: ModuleTreeDataProvider) {
        this.session = session;
        this.treeView = treeView;
    }

    onDidSendMessage(message: any) {
        if (message.type == 'event' && message.event == 'module') {
            let modules = this.treeView.modulesForSession(this.session.id);
            let event = <DebugProtocol.ModuleEvent>message;
            if (event.body.reason == 'new' || event.body.reason == 'changed') {
                let index = modules.findIndex(m => m.id == event.body.module.id);
                if (index == -1) {
                    modules.push(event.body.module);
                } else {
                    modules[index] = event.body.module;
                }
            } else if (event.body.reason == 'removed') {
                modules.filter((m) => m.id != event.body.module.id);
            }

            if (this.session.id == this.treeView.activeSessionId) {
                this.treeView.onDidChangeTreeDataEmitter.fire(null);
            }
        }
    }

    onWillStopSession() {
        delete this.treeView.sessions[this.session.id];
        this.treeView.onDidChangeTreeDataEmitter.fire(null);
    }
}
