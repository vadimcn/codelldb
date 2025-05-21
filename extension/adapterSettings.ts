import { workspace, window, commands, debug, ExtensionContext, ConfigurationScope, StatusBarItem, StatusBarAlignment, QuickPickItem } from "vscode";
import { AdapterSettings } from './novsc/adapterMessages';
import { getExtensionConfig } from "./main";

export class AdapterSettingManager {
    status: StatusBarItem;

    constructor(context: ExtensionContext) {
        let subscriptions = context.subscriptions;

        subscriptions.push(commands.registerCommand('lldb.changeDisplaySettings', () => this.changeDisplaySettings()));

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.displayFormat') ||
                event.affectsConfiguration('lldb.showDisassembly') ||
                event.affectsConfiguration('lldb.dereferencePointers') ||
                event.affectsConfiguration('lldb.suppressMissingSourceFiles') ||
                event.affectsConfiguration('lldb.evaluationTimeout') ||
                event.affectsConfiguration('lldb.consoleMode')) {
                this.propagateDisplaySettings();
            }
        }));

        let registerDisplaySettingCommand = (command: string, updater: (settings: AdapterSettings) => Promise<void>) => {
            context.subscriptions.push(commands.registerCommand(command, async () => {
                let settings = this.getAdapterSettings();
                await updater(settings);
                this.setAdapterSettings(settings);
            }));
        };

        registerDisplaySettingCommand('lldb.toggleConsoleMode', async (settings) => {
            settings.consoleMode = (settings.consoleMode == 'commands') ? 'evaluate' : 'commands';
        });
        registerDisplaySettingCommand('lldb.showDisassembly', async (settings) => {
            settings.showDisassembly = <AdapterSettings['showDisassembly']>await window.showQuickPick(['always', 'auto', 'never']);
        });
        registerDisplaySettingCommand('lldb.toggleDisassembly', async (settings) => {
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
        });
        registerDisplaySettingCommand('lldb.displayFormat', async (settings) => {
            settings.displayFormat = <AdapterSettings['displayFormat']>await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        });
        registerDisplaySettingCommand('lldb.toggleDerefPointers', async (settings) => {
            settings.dereferencePointers = !settings.dereferencePointers;
        });

        this.status = window.createStatusBarItem(StatusBarAlignment.Left, 0);
        this.status.command = 'lldb.changeDisplaySettings';
        this.status.tooltip = 'Change debugger display settings';
        this.status.hide();

        subscriptions.push(debug.onDidChangeActiveDebugSession(session => {
            if (session && session.type == 'lldb')
                this.status.show();
            else
                this.status.hide();
        }));

        this.propagateDisplaySettings();
   }

    // Read current adapter settings values from workspace configuration.
    public getAdapterSettings(scope: ConfigurationScope = undefined): AdapterSettings {
        scope = scope || debug.activeDebugSession?.workspaceFolder;
        let config = getExtensionConfig(scope);
        let settings: AdapterSettings = {
            displayFormat: config.get('displayFormat'),
            showDisassembly: config.get('showDisassembly'),
            dereferencePointers: config.get('dereferencePointers'),
            suppressMissingSourceFiles: config.get('suppressMissingSourceFiles'),
            evaluationTimeout: config.get('evaluationTimeout'),
            consoleMode: config.get('consoleMode'),
            sourceLanguages: null,
            scriptConfig: config.get('script'),
            evaluateForHovers: config.get('evaluateForHovers'),
            commandCompletions: config.get('commandCompletions'),
            reproducer: config.get('reproducer'),
        };
        return settings;
    }

    // Update workspace configuration.
    async setAdapterSettings(settings: AdapterSettings) {
        let folder = debug.activeDebugSession?.workspaceFolder;
        let config = getExtensionConfig(folder);
        await config.update('displayFormat', settings.displayFormat);
        await config.update('showDisassembly', settings.showDisassembly);
        await config.update('dereferencePointers', settings.dereferencePointers);
        await config.update('consoleMode', settings.consoleMode);
    }

    // This is called When configuration change is detected. Updates UI, and if a debug session
    // is active, pushes updated settings to the adapter as well.
    async propagateDisplaySettings() {
        let settings = this.getAdapterSettings();

        this.status.text =
            `Format: ${settings.displayFormat}  ` +
            `Disasm: ${settings.showDisassembly}  ` +
            `Deref: ${settings.dereferencePointers ? 'on' : 'off'}  ` +
            `Console: ${settings.consoleMode == 'commands' ? 'cmd' : 'eval'}`;

        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            await debug.activeDebugSession.customRequest('_adapterSettings', settings);
        }
    }

    // UI for changing display settings.
    async changeDisplaySettings() {
        let settings = this.getAdapterSettings();
        let qpick = window.createQuickPick<QuickPickItem & { command: string }>();
        qpick.items = [
            {
                label: `Value formatting: ${settings.displayFormat}`,
                detail: 'Default format for displaying variable values and evaluation results.',
                command: 'lldb.displayFormat'
            },
            {
                label: `Show disassembly: ${settings.showDisassembly}`,
                detail: 'When to display disassembly.',
                command: 'lldb.showDisassembly'
            },
            {
                label: `Dereference pointers: ${settings.dereferencePointers ? 'on' : 'off'}`,
                detail: 'Whether to show a summary of the pointee or a numeric pointer value.',
                command: 'lldb.toggleDerefPointers'
            },
            {
                label: `Console mode: ${settings.consoleMode}`,
                detail: 'Whether Debug Console input is treated as debugger commands or as expressions to evaluate.',
                command: 'lldb.toggleConsoleMode'
            }
        ];
        qpick.title = 'Debugger display settings';
        qpick.onDidAccept(() => {
            let item = qpick.selectedItems[0];
            qpick.hide();
            commands.executeCommand(item.command);
        });
        qpick.show();
    }
}
