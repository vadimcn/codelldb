import { AdapterSettings, DisplayFormat, ShowDisassembly } from 'codelldb';
import { commands, ConfigurationScope, debug, MarkdownString, StatusBarAlignment, StatusBarItem, window, workspace } from "vscode";
import { getExtensionConfig } from "./main";
import { DisposableSubscriber } from "./novsc/commonTypes";

export class AdapterSettingsManager extends DisposableSubscriber {
    status: StatusBarItem;

    constructor() {
        super();

        this.subscriptions.push(commands.registerCommand('lldb.displayFormat', async () => {
            let settings = this.getAdapterSettings();
            settings.displayFormat = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']) as DisplayFormat;
            if (settings.displayFormat)
                this.setAdapterSettings(settings);
        }));

        this.subscriptions.push(commands.registerCommand('lldb.showDisassembly', async () => {
            let settings = this.getAdapterSettings();
            settings.showDisassembly = await window.showQuickPick(['auto', 'always', 'never']) as ShowDisassembly;
            if (settings.showDisassembly)
                this.setAdapterSettings(settings);
        }));

        this.subscriptions.push(commands.registerCommand('lldb.toggleDisassembly', () => {
            let settings = this.getAdapterSettings();
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
            this.setAdapterSettings(settings);
        }));

        this.subscriptions.push(commands.registerCommand('lldb.toggleConsoleMode', () => {
            let settings = this.getAdapterSettings();
            settings.consoleMode = (settings.consoleMode == 'commands') ? 'evaluate' : 'commands';
            this.setAdapterSettings(settings);
        }));

        this.subscriptions.push(commands.registerCommand('lldb.toggleDerefPointers', () => {
            let settings = this.getAdapterSettings();
            settings.dereferencePointers = !settings.dereferencePointers;
            this.setAdapterSettings(settings);
        }));

        this.subscriptions.push(commands.registerCommand('lldb._updateAdapterSetting', (setting: string, value: any) => {
            let settings = this.getAdapterSettings();
            (settings as any)[setting] = value;
            this.setAdapterSettings(settings);
        }));

        this.status = window.createStatusBarItem(StatusBarAlignment.Left, 0);
        this.status.tooltip = new MarkdownString(this.createSettingsHtml(this.getAdapterSettings()));
        this.status.tooltip.isTrusted = true;
        this.status.tooltip.supportHtml = true;
        this.status.hide();

        this.subscriptions.push(debug.onDidChangeActiveDebugSession(session => {
            if (session && session.type == 'lldb')
                this.status.show();
            else
                this.status.hide();
        }));

        this.propagateDisplaySettings();
        this.subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.displayFormat') ||
                event.affectsConfiguration('lldb.showDisassembly') ||
                event.affectsConfiguration('lldb.dereferencePointers') ||
                event.affectsConfiguration('lldb.suppressMissingSourceFiles') ||
                event.affectsConfiguration('lldb.evaluationTimeout') ||
                event.affectsConfiguration('lldb.consoleMode')) {
                this.propagateDisplaySettings();
            }
        }));
    }

    // Read current adapter settings values from workspace configuration.
    public getAdapterSettings(scope?: ConfigurationScope): AdapterSettings {
        scope = scope ?? debug.activeDebugSession?.workspaceFolder;
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

    createSettingsHtml(settings: AdapterSettings): string {
        function option(setting: string, label: string, value: any) {
            let args = encodeURIComponent(JSON.stringify([setting, value]));
            let opt = `<a href="command:lldb._updateAdapterSetting?${args}">[${label}]</a>`;
            return opt;
        }
        return `<b>Display format:</b> &nbsp;
                    ${option('displayFormat', 'auto', 'auto')} &nbsp;
                    ${option('displayFormat', 'hex', 'hex')} &nbsp;
                    ${option('displayFormat', 'dec', 'decimal')} &nbsp;
                    ${option('displayFormat', 'bin', 'binary')}
                <br>
                <b>Show disassembly:</b> &nbsp;
                    ${option('showDisassembly', 'auto', 'auto')} &nbsp;
                    ${option('showDisassembly', 'always', 'always')} &nbsp;
                    ${option('showDisassembly', 'never', 'never')}
                <br>
                <b>Dereference pointers:</b> &nbsp;
                    ${option('dereferencePointers', 'on', true)} &nbsp;
                    ${option('dereferencePointers', 'off', false)}
                <br>
                <b>Console mode:</b> &nbsp;
                    ${option('consoleMode', 'commands', 'commands')} &nbsp;
                    ${option('consoleMode', 'evaluate', 'evaluate')}
            `;
    }
}
