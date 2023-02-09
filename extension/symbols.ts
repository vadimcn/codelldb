import { window, debug, DebugSession, QuickPickItem, Range, TextEditorRevealType } from 'vscode';
import { Symbol, SymbolsRequest, SymbolsResponse } from './novsc/adapterMessages';

let MAX_SYMBOLS = 1000;

type Item = QuickPickItem & { symbol: Symbol };

export async function pickSymbol(debugSession: DebugSession) {
    let qpick = window.createQuickPick<Item>();
    qpick.matchOnDetail = true;
    qpick.matchOnDescription = true;
    qpick.show();

    async function updateSymbols(filter: string) {
        qpick.busy = true;
        let resp: SymbolsResponse = await debugSession.customRequest('_symbols', <SymbolsRequest>{
            filter: filter,
            maxResults: MAX_SYMBOLS
        });
        let items = resp.symbols.map(symbol => <Item>{
            label: symbol.name.length > 0 ? symbol.name : '<no name>',
            detail: `${symbol.type} @ ${symbol.address}`,
            symbol: symbol
        });
        qpick.items = items;
        if (items.length == MAX_SYMBOLS)
            qpick.title = 'Too many matching symbols, please refine your query.';
        else
            qpick.title = null;
        qpick.busy = false;
    }

    // Delay updates by 500ms
    let pendingUpdate: any = null;
    qpick.onDidChangeValue((filter) => {
        if (pendingUpdate)
            clearTimeout(pendingUpdate);
        pendingUpdate = setTimeout(updateSymbols, 500, filter);
    });

    qpick.onDidHide(() => {
        if (pendingUpdate)
            clearTimeout(pendingUpdate);
        qpick.busy = false;
    });

    qpick.onDidAccept(async () => {
        let symbol = qpick.selectedItems[0].symbol;
        if (symbol.location) {
            let uri = debug.asDebugSourceUri(symbol.location[0], debugSession);
            let editor = await window.showTextDocument(uri, { preserveFocus: true, preview: true });
            let line = <number>symbol.location[1];
            editor.revealRange(new Range(line - 1, 0, line, 0), TextEditorRevealType.AtTop);
        } else if (symbol.type == 'Code') {
            await debugSession.customRequest('evaluate', { context: '_command', expression: `disassemble -s ${symbol.address}` });
        } else {
            await debugSession.customRequest('evaluate', { context: '_command', expression: `memory read ${symbol.address}` });
        }
    });
}
