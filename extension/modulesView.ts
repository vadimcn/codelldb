import {
    debug, env, commands, TreeDataProvider, TreeItem, ProviderResult, EventEmitter,
    TreeItemCollapsibleState, DebugSession, DebugAdapterTracker, DebugAdapterTrackerFactory
} from 'vscode';
import { DisposableSubscriber, MapEx } from './novsc/commonTypes';
import { DebugProtocol } from '@vscode/debugprotocol';

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

export class ModuleTreeDataProvider extends DisposableSubscriber
    implements TreeDataProvider<Element>, DebugAdapterTrackerFactory {

    sessions = new MapEx<string, Module[]>();
    activeSessionId: string | undefined;

    onDidChangeTreeDataEmitter = new EventEmitter<any>();
    readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

    constructor() {
        super();
        this.subscriptions.push(commands.registerCommand('lldb.modules.copyValue', (arg) => this.copyValue(arg)));
        this.subscriptions.push(debug.registerDebugAdapterTrackerFactory('lldb', this));
        this.subscriptions.push(debug.onDidStartDebugSession(this.onStartDebugSession, this));
        this.subscriptions.push(debug.onDidChangeActiveDebugSession(this.onChangedActiveDebugSession, this));
    }

    copyValue(prop: ModuleProperty): Thenable<void> {
        return env.clipboard.writeText(prop.value);
    }

    moduleChanged(session: DebugSession, event: DebugProtocol.ModuleEvent) {
        let modules = this.sessions.setdefault(session.id, []);
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

        if (session.id == this.activeSessionId) {
            this.onDidChangeTreeDataEmitter.fire(null);
        }
    }

    onStartDebugSession(session: DebugSession) {
        if (!this.activeSessionId) {
            this.activeSessionId = session?.id;
            this.onDidChangeTreeDataEmitter.fire(null);
        }
    }

    onChangedActiveDebugSession(session: DebugSession | undefined) {
        this.activeSessionId = session?.id;
        this.onDidChangeTreeDataEmitter.fire(null);
    }

    // TreeDataProvider
    getChildren(element?: Element): ProviderResult<Element[]> {
        if (element == undefined) {
            return (this.activeSessionId ? this.sessions.get(this.activeSessionId) : null) || [];
        } else if (element instanceof ModuleProperty) {
            return [];
        } else {
            let module = element as Module;
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
            let module = element as Module;
            let item = new TreeItem(module.name, TreeItemCollapsibleState.Collapsed);
            return item;
        }
    }

    // DebugAdapterTrackerFactory
    createDebugAdapterTracker(session: DebugSession): ProviderResult<DebugAdapterTracker> {
        let treeView = this;
        return {
            onDidSendMessage(message: any) {
                if (message.type == 'event' && message.event == 'module') {
                    treeView.moduleChanged(session, message as DebugProtocol.ModuleEvent);
                }
            },
            onWillStopSession() {
                treeView.sessions.delete(session.id);
                treeView.onDidChangeTreeDataEmitter.fire(null);
            }
        }
    }
}
