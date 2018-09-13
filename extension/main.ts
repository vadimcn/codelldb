import {
    workspace, window, commands, debug,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration,
} from 'vscode';
import { inspect } from 'util';
import { ChildProcess } from 'child_process';
import * as path from 'path';
import * as diagnostics from './diagnostics';
import * as htmlView from './htmlView';
import * as cargo from './cargo';
import * as util from './util';
import * as adapter from './adapter';
import * as install from './install';
import { Dict, AdapterType, toAdapterType } from './common';

export let output = window.createOutputChannel('LLDB');

// Main entry point
export function activate(context: ExtensionContext) {
    new Extension(context);
}

class Extension implements DebugConfigurationProvider {
    context: ExtensionContext;
    htmlViewer: htmlView.DebuggerHtmlView;

    constructor(context: ExtensionContext) {
        this.context = context;
        this.htmlViewer = new htmlView.DebuggerHtmlView(context);

        let subscriptions = context.subscriptions;
        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));

        subscriptions.push(commands.registerCommand('lldb.diagnose', () => this.runDiagnostics()));
        subscriptions.push(commands.registerCommand('lldb.getCargoLaunchConfigs', () => this.getCargoLaunchConfigs()));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', () => this.pickProcess(false)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', () => this.pickProcess(true)));

        this.registerDisplaySettingCommand('lldb.showDisassembly', async (settings) => {
            settings.showDisassembly = await window.showQuickPick(['always', 'auto', 'never']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDisassembly', async (settings) => {
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
        });
        this.registerDisplaySettingCommand('lldb.displayFormat', async (settings) => {
            settings.displayFormat = await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDerefPointers', async (settings) => {
            settings.dereferencePointers = !settings.dereferencePointers;
        });
        this.registerDisplaySettingCommand('lldb.toggleContainerSummary', async (settings) => {
            settings.containerSummary = !settings.containerSummary;
        });
    }

    registerDisplaySettingCommand(command: string, updater: (settings: DisplaySettings) => Promise<void>) {
        this.context.subscriptions.push(commands.registerCommand(command, async () => {
            if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
                let settings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
                await updater(settings);
                this.context.globalState.update('display_settings', settings);
                await debug.activeDebugSession.customRequest('displaySettings', settings);
            }
        }));
    }

    async provideDebugConfigurations(
        folder: WorkspaceFolder | undefined,
        token?: CancellationToken
    ): Promise<DebugConfiguration[]> {
        try {
            let debugConfigs = await cargo.getLaunchConfigs(folder ? folder.uri.fsPath : workspace.rootPath);
            if (debugConfigs.length > 0) {
                let response = await window.showInformationMessage(
                    'Cargo.toml has been detected in this workspace.\r\n' +
                    'Would you like to generate launch configurations for its targets?', { modal: true }, 'Yes', 'No');
                if (response == 'Yes') {
                    return debugConfigs;
                }
            }
        } catch (err) {
            output.appendLine(err.toString());
        }

        return [{
            type: 'lldb',
            request: 'launch',
            name: 'Debug',
            program: '${workspaceFolder}/<your program>',
            args: [],
            cwd: '${workspaceFolder}'
        }];
    }

    // Invoked by VSCode to initiate a new debugging session.
    async resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        launchConfig: DebugConfiguration,
        token?: CancellationToken
    ): Promise<DebugConfiguration> {
        output.clear();

        if (launchConfig.type === undefined) {
            await window.showErrorMessage('Cannot start debugging because no launch configuration has been provided.', { modal: true });
            return null;
        }

        if (!await this.checkPrerequisites(folder))
            return undefined;

        let launchDefaults = workspace.getConfiguration('lldb.launch', folder ? folder.uri : undefined);
        launchConfig = this.mergeWorkspaceSettings(launchDefaults, launchConfig);

        let dbgconfigConfig = workspace.getConfiguration('lldb.dbgconfig', folder ? folder.uri : undefined);
        launchConfig = util.expandDbgConfig(launchConfig, dbgconfigConfig);

        // Transform "request":"custom" to "request":"launch" + "custom":true
        if (launchConfig.request == 'custom') {
            launchConfig.request = 'launch';
            launchConfig.custom = true;
        }

        // Deal with Cargo
        let cargoDict: Dict<string> = {};
        if (launchConfig.cargo != undefined) {
            let cargoCwd = folder ? folder.uri.fsPath : workspace.rootPath;
            cargoDict.program = await cargo.getProgramFromCargo(launchConfig.cargo, cargoCwd);
            delete launchConfig.cargo;

            // Expand ${cargo:program}.
            launchConfig = cargo.expandCargo(launchConfig, cargoDict);

            if (launchConfig.program == undefined) {
                launchConfig.program = cargoDict.program;
            }

            // Add 'rust' to sourceLanguages, since this project obviously (ha!) involves Rust.
            if (!launchConfig.sourceLanguages)
                launchConfig.sourceLanguages = [];
            launchConfig.sourceLanguages.push('rust');
        }

        let lldbConfig = workspace.getConfiguration('lldb', folder ? folder.uri : undefined);
        let adapterParams: any = this.getAdapterParameters(lldbConfig);
        if (launchConfig.sourceLanguages) {
            adapterParams.sourceLanguages = launchConfig.sourceLanguages;
            delete launchConfig.sourceLanguages;
        }

        output.appendLine('Starting new session with:');
        output.appendLine(inspect(launchConfig));

        try {
            // If configuration does not provide debugServer explicitly, launch new adapter.
            if (!launchConfig.debugServer) {
                let [adapter, port] = await this.startDebugAdapter(folder, adapterParams);
                launchConfig.debugServer = port;
            }
            launchConfig._displaySettings = this.context.globalState.get<DisplaySettings>('display_settings') || new DisplaySettings();
            return launchConfig;
        } catch (err) {
            diagnostics.analyzeStartupError(err, this.context, output);
            return null;
        }
    }

    // Merge launch configuration with workspace settings
    mergeWorkspaceSettings(launchConfig: WorkspaceConfiguration, debugConfig: DebugConfiguration): DebugConfiguration {
        let mergeConfig = (key: string, reverse: boolean = false) => {
            let value1 = util.getConfigNoDefault(launchConfig, key);
            let value2 = debugConfig[key];
            let value = !reverse ? util.mergeValues(value1, value2) : util.mergeValues(value2, value1);
            if (!util.isEmpty(value))
                debugConfig[key] = value;
        }
        mergeConfig('initCommands');
        mergeConfig('preRunCommands');
        mergeConfig('postRunCommands');
        mergeConfig('exitCommands', true);
        mergeConfig('env');
        mergeConfig('cwd');
        mergeConfig('terminal');
        mergeConfig('stdio');
        mergeConfig('expressions');
        mergeConfig('sourceMap');
        mergeConfig('sourceLanguages');
        mergeConfig('debugServer');
        return debugConfig;
    }

    getAdapterParameters(config: WorkspaceConfiguration, params: Dict<any> = {}): Dict<any> {
        util.setIfDefined(params, config, 'reverseDebugging');
        util.setIfDefined(params, config, 'suppressMissingSourceFiles');
        util.setIfDefined(params, config, 'evaluationTimeout');
        return params;
    }

    async getCargoLaunchConfigs() {
        try {
            let debugConfigs = await cargo.getLaunchConfigs(workspace.rootPath);
            let doc = await workspace.openTextDocument({
                content: JSON.stringify(debugConfigs, null, 4),
                language: 'jsonc'
            });
            await window.showTextDocument(doc, 1, false);
        } catch (err) {
            output.show();
            window.showErrorMessage(err.toString());
        }
    }

    async pickProcess(currentUserOnly: boolean): Promise<string> {
        let items = util.getProcessList(currentUserOnly);
        let item = await window.showQuickPick(items);
        if (item) {
            return item.pid.toString();
        } else {
            return undefined;
        }
    }

    async startDebugAdapter(
        folder: WorkspaceFolder | undefined,
        params: Dict<string>
    ): Promise<[ChildProcess, number]> {
        let config = workspace.getConfiguration('lldb', folder ? folder.uri : undefined);
        let adapterType = this.getAdapterType(folder);
        let adapterEnv = config.get('executable_env', {});
        let verboseLogging = config.get<boolean>('verboseLogging');
        let adapterProcess;
        if (adapterType == 'classic') {
            adapterProcess = await adapter.startClassic(
                this.context.extensionPath,
                config.get('executable', 'lldb'),
                adapterEnv,
                workspace.rootPath,
                params,
                verboseLogging);
        } else if (adapterType == 'bundled') {
            adapterProcess = await adapter.startClassic(
                this.context.extensionPath,
                path.join(this.context.extensionPath, 'lldb/bin/lldb'),
                adapterEnv,
                workspace.rootPath,
                params,
                verboseLogging);
        } else {
            adapterProcess = await adapter.startNative(
                this.context.extensionPath,
                path.join(this.context.extensionPath, 'lldb'),
                adapterEnv,
                workspace.rootPath,
                params,
                verboseLogging);
        };
        util.logProcessOutput(adapterProcess, output);
        let port = await adapter.getDebugServerPort(adapterProcess);
        return [adapterProcess, port];
    }

    async checkPrerequisites(folder: WorkspaceFolder | undefined): Promise<boolean> {
        if (this.getAdapterType(folder) == 'classic') {
            if (!this.context.globalState.get('lldb_works')) {
                window.showInformationMessage("Since this is the first time you are starting LLDB, I'm going to run some quick diagnostics...");
                if (!await diagnostics.diagnoseExternalLLDB(this.context, output))
                    return false;
                this.context.globalState.update('lldb_works', true);
            }
        } else {
            if (!await diagnostics.checkPython(output))
                return false;
            if (!await install.ensurePlatformPackage(this.context, output))
                return false;
        }
        return true;
    }

    async runDiagnostics() {
        let adapterType = this.getAdapterType(undefined);
        let succeeded;
        switch (adapterType) {
            case 'classic':
                succeeded = await diagnostics.diagnoseExternalLLDB(this.context, output);
                break;
            case 'bundled':
            case 'native':
                succeeded = await diagnostics.checkPython(output);
                break;
        }
        if (succeeded) {
            window.showInformationMessage('LLDB self-test completed successfuly.');
        }
    }

    getAdapterType(folder: WorkspaceFolder | undefined): AdapterType {
        let lldbConfig = workspace.getConfiguration('lldb', folder ? folder.uri : undefined);
        return toAdapterType(lldbConfig.get<string>('adapterType'));
    }
}

class DisplaySettings {
    showDisassembly: string = 'auto'; // 'always' | 'auto' | 'never'
    displayFormat: string = 'auto'; // 'auto' | 'hex' | 'decimal' | 'binary'
    dereferencePointers: boolean = true;
    containerSummary: boolean = true;
};
