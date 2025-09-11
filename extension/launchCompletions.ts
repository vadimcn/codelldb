import {
    CancellationToken, CompletionItem, CompletionItemKind, CompletionItemProvider, DebugConfiguration, Position,
    RelativePattern, Range, SnippetString, TextDocument, window, workspace, WorkspaceEdit, WorkspaceFolder
} from "vscode";
import path from "node:path";
import * as jsonc from 'jsonc-parser';

export class LaunchCompletionProvider implements CompletionItemProvider {

    provideDebugConfiguration;

    constructor(provideDebugConfiguration: (
        workspaceFolder?: WorkspaceFolder,
        cancellation?: CancellationToken) => Promise<DebugConfiguration | unknown>
    ) {
        this.provideDebugConfiguration = provideDebugConfiguration;
    }

    public async provideCompletionItems(
        document: TextDocument,
        position: Position,
        token: CancellationToken,
    ): Promise<CompletionItem[]> {
        if (path.basename(document.uri.fsPath) != 'launch.json')
            return [];
        let folder = workspace.getWorkspaceFolder(document.uri);
        if (!folder)
            return [];
        let files = await workspace.findFiles(new RelativePattern(folder, '**/Cargo.toml'), null, 1);
        if (files.length == 0)
            return [];
        let docText = document.getText();
        let docOffset = document.offsetAt(position);
        let location = jsonc.getLocation(docText, docOffset);
        if (location.path[0] != 'configurations' || location.path.length != 2)
            return [];
        // If positioned at a comma following an item, insert after that item.
        if (docText[docOffset] == ',' && typeof location.path[1] == 'number')
            location.path[1] += 1;

        return [
            {
                kind: CompletionItemKind.Enum,
                label: 'CodeLLDB: Cargo ...',
                sortText: 'CodeLLDB:',
                documentation: 'Select Cargo configuration.',
                insertText: new SnippetString(),
                command: {
                    command: 'lldb.insertDebugConfig',
                    title: 'insertDebugConfig',
                    arguments: [document, location.path, token],
                },
            }
        ];
    }

    async insertDebugConfig(args: any[]) {
        let [document, jsonPath, token] = args as [TextDocument, jsonc.JSONPath, CancellationToken];
        if (window.activeTextEditor?.document !== document)
            return;
        let config = await this.provideDebugConfiguration(workspace.getWorkspaceFolder(document.uri), token);
        if (config) {
            let editOptions = window.activeTextEditor.options;
            let formattingOptions = {
                insertSpaces: editOptions.insertSpaces as boolean ?? true,
                tabSize: editOptions.tabSize as number ?? 4,
                insertFinalNewline: true
            };
            let modifications = jsonc.modify(document.getText(), jsonPath, config, {
                isArrayInsertion: true,
                formattingOptions: formattingOptions,
            });
            let edit = new WorkspaceEdit();
            for (let mod of modifications) {
                let pos = document.positionAt(mod.offset);
                let endPos = document.positionAt(mod.offset + mod.length);
                edit.delete(document.uri, new Range(pos, endPos));
                edit.insert(document.uri, pos, mod.content);
            }
            workspace.applyEdit(edit);
        }
    }
}
