import { DebugSession, window, QuickPickItem } from 'vscode';
import { SymbolsRequest, SymbolsResponse } from './adapterMessages';

export async function pickSymbol(debugSession: DebugSession) {
    let qpick = window.createQuickPick();
    qpick.matchOnDetail = true;
    qpick.matchOnDescription = true;
    qpick.show();
    qpick.onDidHide(() => qpick.busy = false);

    let items: QuickPickItem[] = [];
    let continuationToken = null;
    qpick.busy = true;
    do {
        let resp: SymbolsResponse = await debugSession.customRequest('_symbols', <SymbolsRequest>{ continuationToken })
        items = items.concat(resp.symbols.map(s => <QuickPickItem>{
            label: s.name.length > 0 ? s.name : '<no name>',
            detail: `${s.type} @ ${s.address}`,
        }));
        qpick.items = items;
        continuationToken = resp.continuationToken;
    } while (continuationToken != null && qpick.busy);
    qpick.busy = false;
}
