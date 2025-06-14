import {
    window, debug, DebugSession, DebugSessionCustomEvent, WebviewPanel, ViewColumn
} from "vscode";
import { Dict, DisposableSubscriber } from './novsc/commonTypes';

interface DebuggerPanel extends WebviewPanel {
    preserveOrphaned: boolean
}

export class WebviewManager extends DisposableSubscriber {
    sessionPanels: Dict<Dict<DebuggerPanel>> = {};

    constructor() {
        super();
        this.subscriptions.push(debug.onDidTerminateDebugSession(this.onTerminatedDebugSession, this));
        this.subscriptions.push(debug.onDidReceiveDebugSessionCustomEvent(this.onDebugSessionCustomEvent, this));
    }

    onTerminatedDebugSession(session: DebugSession) {
        if (session.type == 'lldb') {
            let panels = this.sessionPanels[session.id];
            if (panels) {
                for (let panel of Object.values(panels)) {
                    if (!panel.preserveOrphaned)
                        panel.dispose();
                }
            }
        }
    }

    onDebugSessionCustomEvent(e: DebugSessionCustomEvent) {
        if (e.session.type == 'lldb') {
            if (e.event == '_pythonMessage') {
                if (e.body.message == 'webviewCreate') {
                    this.createWebview(e.session, e.body);
                } else if (e.body.message == 'webviewDispose') {
                    this.sessionPanels[e.session.id][e.body.id].dispose();
                } else if (e.body.message == 'webviewSetHtml') {
                    this.sessionPanels[e.session.id][e.body.id].webview.html = e.body.html;
                } else if (e.body.message == 'webviewReveal') {
                    this.sessionPanels[e.session.id][e.body.id].reveal(e.body.viewColumn, e.body.preserveFocus)
                } else if (e.body.message == 'webviewPostMessage') {
                    this.sessionPanels[e.session.id][e.body.id].webview.postMessage(e.body.inner);
                }
            }
        }
    }

    createWebview(session: DebugSession, body: any) {
        let view_id = body.id;
        let panel = <DebuggerPanel>window.createWebviewPanel(
            'codelldb.webview',
            body.title || session.name,
            {
                viewColumn: body.viewColumn != null ? body.viewColumn : ViewColumn.Active,
                preserveFocus: body.preserveFocus
            },
            {
                enableFindWidget: body.enableFindWidget,
                enableScripts: body.enableScripts,
                retainContextWhenHidden: body.retainContextWhenHidden
            }
        );
        panel.webview.onDidReceiveMessage(e => {
            session.customRequest('_pythonMessage', { message: 'webviewDidReceiveMessage', id: view_id, inner: e });
        });
        panel.onDidDispose(() => {
            delete this.sessionPanels[session.id][view_id];
            session.customRequest('_pythonMessage', { message: 'webviewDidDispose', id: view_id });
        });
        if (body.html)
            panel.webview.html = body.html;
        panel.preserveOrphaned = body.preserveOrphaned

        let panels = this.sessionPanels[session.id];
        if (panels == undefined) {
            panels = {};
            this.sessionPanels[session.id] = panels;
        }
        panels[view_id] = panel;
    }
}

