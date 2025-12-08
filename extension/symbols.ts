import { window, debug, DebugSession, QuickPickItem, Range, TextEditorRevealType, DebugProtocolSource, ThemeIcon } from 'vscode';
import { Symbol, SymbolsRequest, SymbolsResponse } from 'codelldb';
import * as path from 'node:path';

let MAX_SYMBOLS = 1000;

type Item = QuickPickItem & { symbol?: Symbol };

export async function pickSymbol(debugSession: DebugSession | undefined) {
    if (debugSession?.type != 'lldb') {
        return;
    }

    let qpick = window.createQuickPick<Item>();
    qpick.title = `Searching symbols of ${debugSession.name}`;
    qpick.placeholder = 'Symbol name';
    qpick.matchOnDetail = true;
    qpick.matchOnDescription = true;
    qpick.show();

    let updateSymbols = async function (filter: string) {
        qpick.busy = true;
        let resp: SymbolsResponse = await debugSession.customRequest('_symbols', {
            filter: filter,
            maxResults: MAX_SYMBOLS
        } satisfies SymbolsRequest);
        let items: Item[] = resp.symbols.map(symbol => {
            let icon;
            if (symbol.type == 'Code' || symbol.type == 'Trampoline') {
                icon = new ThemeIcon('symbol-function');
            } else if (symbol.type == 'Data') {
                icon = new ThemeIcon('symbol-constant');
            } else {
                icon = new ThemeIcon('symbol-misc');
            }
            let moduleName = symbol.module ? path.basename(symbol.module) : '<unknown>';
            return {
                label: symbol.name.length > 0 ? symbol.name : '<no name>',
                detail: `${symbol.type}, module: ${moduleName}, address: ${symbol.address}, size: ${symbol.size}`,
                iconPath: icon,
                symbol: symbol,
            }
        });
        if (items.length >= MAX_SYMBOLS) {
            items.push({
                label: '',
                detail: 'Too many matching symbols, please refine your query.',
                iconPath: new ThemeIcon('ellipsis'),
                symbol: undefined,
                alwaysShow: true,
            });
        }
        qpick.items = items;
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
        if (symbol) {
            if (symbol.location) {
                let uri = debug.asDebugSourceUri(symbol.location[0] as DebugProtocolSource, debugSession);
                let editor = await window.showTextDocument(uri, { preserveFocus: true, preview: true });
                let line = symbol.location[1] as number;
                editor.revealRange(new Range(line - 1, 0, line, 0), TextEditorRevealType.AtTop);
            } else if (symbol.type == 'Code' || symbol.type == 'Trampoline') {
                await debugSession.customRequest('evaluate', { context: '_command', expression: `disassemble -s ${symbol.address}` });
            } else {
                await debugSession.customRequest('evaluate', { context: '_command', expression: `memory read ${symbol.address}` });
            }
        }
    });
}
