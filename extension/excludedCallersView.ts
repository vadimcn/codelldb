import {
    debug, window, commands, TreeDataProvider, TreeItem, ProviderResult, EventEmitter, ExtensionContext,
    DebugSession, Breakpoint, TreeItemCollapsibleState, SourceBreakpoint, BreakpointsChangeEvent
} from 'vscode';
import { MapEx } from './novsc/commonTypes';
import { ExcludeCallerRequest, ExcludeCallerResponse, SetExcludedCallersRequest } from './novsc/adapterMessages';
import { DebugProtocol } from '@vscode/debugprotocol';
import { ThemeIcon } from 'vscode';
import { workspace } from 'vscode';


interface Exclusion {
    symbol: string;
}

type Element = Breakpoint | string | Exclusion; // strings represent exceptions

export class ExcludedCallersView implements TreeDataProvider<Element> {

    context: ExtensionContext;
    breakpointExclusions = new MapEx<string, Exclusion[]>; // Exclusions indexed by breakpoint id
    exceptionExclusions = new MapEx<string, [string, Exclusion[]]>; // Exclusions indexed by exception filter id

    onDidChangeTreeDataEmitter = new EventEmitter<any>();
    readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

    constructor(context: ExtensionContext) {
        this.context = context;
        context.subscriptions.push(commands.registerCommand('lldb.excludedCallers.add', (s, f) => this.excludeCaller(f)));
        context.subscriptions.push(commands.registerCommand('lldb.excludedCallers.remove', item => this.removeExclusion(item)));
        context.subscriptions.push(commands.registerCommand('lldb.excludedCallers.removeAll', _ => this.removeExclusion(null)));
        context.subscriptions.push(debug.registerDebugAdapterTrackerFactory('lldb', this));
        context.subscriptions.push(debug.onDidChangeBreakpoints(e => this.breakpointsChanged(e)));
    }

    breakpointsChanged(event: BreakpointsChangeEvent) {
        for (let bp of event.removed) {
            this.breakpointExclusions.delete(bp.id);
        }
        this.onDidChangeTreeDataEmitter.fire(null);
    }

    async excludeCaller(frame: any) {
        let session = debug.activeDebugSession;
        if (session.type == 'lldb' && frame.frameLocation != undefined) {
            try {
                let resp = await session.customRequest('_excludeCaller', <ExcludeCallerRequest>{
                    source: frame.frameLocation.source.path || frame.frameLocation.source.sourceReference,
                    line: frame.frameLocation.range.startLineNumber,
                    column: frame.frameLocation.range.startColumn,
                }) as ExcludeCallerResponse;

                // If adapter responded with a number, it's a breakpoint id, which we need to cnvert to a stable
                // debug.Breakpoint id.  A string means the last breakpoint was an exception breakpoint.
                let exclusions: Exclusion[] = null;
                if (typeof resp.breakpointId == 'number') {
                    for (let bp of debug.breakpoints) {
                        let dbp = await session.getDebugProtocolBreakpoint(bp) as DebugProtocol.Breakpoint;
                        if (dbp && dbp.id == resp.breakpointId) {
                            exclusions = this.breakpointExclusions.setdefault(bp.id, []);
                            break;
                        }
                    }
                } else {
                    let [excName, label] = resp.breakpointId;
                    // First element of the value tuple is the display label of the exception.
                    exclusions = this.exceptionExclusions.setdefault(excName, [label, []])[1];
                }
                if (exclusions && !exclusions.find(e => e.symbol == resp.symbol)) {
                    exclusions.push({ symbol: resp.symbol });
                    this.onDidChangeTreeDataEmitter.fire(null);
                }
            } catch (e) {
                await window.showErrorMessage(e.message);
            }
        }
        this.saveState();
    }

    async removeExclusion(item: Element) {
        if (item == null) {
            this.exceptionExclusions.clear();
            this.breakpointExclusions.clear();
        } else if (item instanceof Breakpoint) {
            this.breakpointExclusions.delete(item.id);
        } else if (typeof item == 'string') {
            this.exceptionExclusions.delete(item);
        } else {
            function filterMap(map: Map<string, any>, val2exc: (val: any) => Exclusion[], item: Exclusion) {
                for (let [key, val] of map.entries()) {
                    let exclusions = val2exc(val);
                    let idx = exclusions.indexOf(item);
                    if (idx != -1) {
                        exclusions.splice(idx, 1);
                        if (exclusions.length == 0)
                            map.delete(key);
                    }
                }
            };
            filterMap(this.breakpointExclusions, val => val, item);
            filterMap(this.exceptionExclusions, val => val[1], item);

        }
        this.saveState();
        this.onDidChangeTreeDataEmitter.fire(null);
        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            await this.sendExcludedCallers(debug.activeDebugSession);
        }
    }

    // Send the list of relevant exclusions to the debug session.
    async sendExcludedCallers(session: DebugSession) {
        let adapterExclusions = [];

        for (let bp of debug.breakpoints) {
            let exclusions = this.breakpointExclusions.get(bp.id);
            if (exclusions) {
                let dbp = await session.getDebugProtocolBreakpoint(bp) as DebugProtocol.Breakpoint;
                if (dbp) {
                    for (let exclusion of exclusions) {
                        adapterExclusions.push({
                            breakpointId: dbp.id,
                            symbol: exclusion.symbol
                        });
                    }
                }
            }
        }

        for (let excName of this.exceptionExclusions.keys()) {
            for (let exclusion of this.exceptionExclusions.get(excName)[1]) {
                adapterExclusions.push({
                    breakpointId: excName,
                    symbol: exclusion.symbol
                });
            }
        }

        if (adapterExclusions.length > 0) {
            await session.customRequest('_setExcludedCallers', {
                exclusions: adapterExclusions
            } as SetExcludedCallersRequest);
        }
    }

    async saveState() {
        let bpids = new Set<string>(debug.breakpoints.map(bp => bp.id));
        let state = {
            breakpointExclusions: Array.from(this.breakpointExclusions.entries())
                .filter(([key, val]) => bpids.has(key) && val.length > 0),
            exceptionExclusions: Array.from(this.exceptionExclusions.entries())
                .filter(([key, val]) => val.length > 0),
        };
        await this.context.workspaceState.update("lldb.excludedCallers", state);
    }

    loadState() {
        try {
            let state: any = this.context.workspaceState.get("lldb.excludedCallers");
            if (state) {
                this.breakpointExclusions.clear();
                for (let [key, val] of state.breakpointExclusions) {
                    this.breakpointExclusions.set(key, val);
                }
                this.exceptionExclusions.clear();
                for (let [key, val] of state.exceptionExclusions) {
                    this.exceptionExclusions.set(key, val);
                }
            }
        }
        catch (err) {
            console.error(err);
        }
    }

    // TreeDataProvider
    getChildren(element: Element): ProviderResult<Element[]> {
        if (element == undefined) {
            // Root
            let items: Element[] = [];
            for (let excName of this.exceptionExclusions.keys()) {
                items.push(excName)
            }
            for (let bp of debug.breakpoints) {
                if (this.breakpointExclusions.get(bp.id)) {
                    items.push(bp);
                }
            }
            return items;
        } else if (typeof element == 'string') {
            // Exception
            return this.exceptionExclusions.get(element)[1];
        } else {
            // Breakpoint
            let bp = element as Breakpoint;
            return this.breakpointExclusions.get(bp.id);
        }
    }

    getTreeItem(element: Element): TreeItem {
        if (typeof element == 'string') {
            // Exception
            let label = this.exceptionExclusions.get(element)[0];
            let item = new TreeItem(label, TreeItemCollapsibleState.Expanded);
            item.iconPath = new ThemeIcon('zap');
            return item;
        } if (element instanceof Breakpoint) {
            // Breakpoint
            let item = new TreeItem(element.id, TreeItemCollapsibleState.Expanded);
            item.iconPath = new ThemeIcon('circle-outline');
            if (element instanceof SourceBreakpoint) {
                let path = element.location.uri.path;
                item.label = path.substring(path.lastIndexOf('/') + 1) + ':' + element.location.range.start.line;
                item.tooltip = element.location.uri.fsPath || element.location.uri.toString();
                item.command = {
                    title: 'Show',
                    command: 'vscode.open',
                    arguments: [element.location.uri, { selection: element.location.range }]
                };
            }
            return item;
        } else {
            // Caller
            let item = new TreeItem(element.symbol);
            item.iconPath = new ThemeIcon('exclude');
            return item;
        }
    }

    // DebugAdapterTrackerFactory
    createDebugAdapterTracker(session: DebugSession) {
        if (session.type == 'lldb') {
            let provider = this;
            return {
                async onWillReceiveMessage(message: DebugProtocol.ProtocolMessage) {
                    if (message.type == 'request' && (message as DebugProtocol.Request).command == 'configurationDone') {
                        provider.sendExcludedCallers(session);
                    }
                }
            }
        }
    }
}
