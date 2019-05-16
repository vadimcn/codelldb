import {
    workspace, window, commands, debug, QuickPickOptions,
    ExtensionContext, WorkspaceConfiguration, WorkspaceFolder, CancellationToken,
    DebugConfigurationProvider, DebugConfiguration, DebugAdapterDescriptorFactory, DebugSession, DebugAdapterExecutable,
    DebugAdapterDescriptor, DebugAdapterServer, extensions, Uri, StatusBarAlignment, QuickPickItem, StatusBarItem, ConfigurationChangeEvent,
} from 'vscode';
import { DebugProtocol } from 'vscode-debugprotocol';
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
import { DisplaySettings } from './adapterMessages';
import { execFileAsync } from './async';

export let output = window.createOutputChannel('LLDB');

// Main entry point
export function activate(context: ExtensionContext) {
    let extension = new Extension(context);
    extension.onActivate();
}

class SetNextStatementHelpers
{
    public static makeLabelsUnique(targets: DebugProtocol.GotoTarget[]) : { [key: string]: DebugProtocol.GotoTarget } {

        // first try: use the original label names
        let labelDict : { [key: string]: DebugProtocol.GotoTarget } | undefined = SetNextStatementHelpers.makeLabelDictorary(targets);
        if (!labelDict) {
            // next try to add on the source position
            labelDict = SetNextStatementHelpers.tryMakeLabelsUnique(targets, (target: DebugProtocol.GotoTarget) => `${target.label} : source position (${target.line},${target.column})-(${target.endLine},${target.endColumn})`);
            if (!labelDict) {
                // nothing worked, so just add on the array index as a prefix
                labelDict = SetNextStatementHelpers.tryMakeLabelsUnique(targets, (target: DebugProtocol.GotoTarget, index: number) => `${index+1}: ${target.label}`);
            }
        }

        return labelDict;
    }

    static tryMakeLabelsUnique(targets: DebugProtocol.GotoTarget[], getLabel: (target: DebugProtocol.GotoTarget, index?:number) => string) : { [key: string]: DebugProtocol.GotoTarget } | undefined {
        const labelDict = SetNextStatementHelpers.makeLabelDictorary(targets, getLabel);
        if (!labelDict) {
            // The specified 'getLabel' function wasn't able to make the label names unique
            return undefined;
        }

        // The specified 'getLabel' fenction worked. Update the 'label' names in the 'targets' array.
        targets.forEach((target, index) => {
            target.label = getLabel(target, index);
        });
        return labelDict;
    }

    static makeLabelDictorary(targets: DebugProtocol.GotoTarget[], getLabel?: (target: DebugProtocol.GotoTarget, index?:number) => string) : { [key: string]: DebugProtocol.GotoTarget } | undefined {
        if (!getLabel) {
            getLabel = (target) => target.label;
        }

        const labelNameDict : { [key: string]: DebugProtocol.GotoTarget } = {};
        let index:number = 0;
        for (const target of targets) {
            const key:string = getLabel(target, index);
            let existingItem = labelNameDict[key];
            if (existingItem !== undefined) {
                // multiple values with the same label found
                return undefined;
            }
            labelNameDict[key] = target;
            index++;
        }

        return labelNameDict;
    }
}

class Extension implements DebugConfigurationProvider, DebugAdapterDescriptorFactory {
    context: ExtensionContext;
    htmlViewer: htmlView.DebuggerHtmlView;
    status: StatusBarItem;

    constructor(context: ExtensionContext) {
        this.context = context;
        this.htmlViewer = new htmlView.DebuggerHtmlView(context);

        let subscriptions = context.subscriptions;

        subscriptions.push(debug.registerDebugConfigurationProvider('lldb', this));
        subscriptions.push(debug.registerDebugAdapterDescriptorFactory('lldb', this));

        subscriptions.push(commands.registerCommand('lldb.diagnose', () => this.runDiagnostics()));
        subscriptions.push(commands.registerCommand('lldb.getCargoLaunchConfigs', () => this.getCargoLaunchConfigs()));
        subscriptions.push(commands.registerCommand('lldb.pickProcess', () => this.pickProcess(false)));
        subscriptions.push(commands.registerCommand('lldb.pickMyProcess', () => this.pickProcess(true)));
        subscriptions.push(commands.registerCommand('lldb.changeDisplaySettings', () => this.changeDisplaySettings()));
        subscriptions.push(commands.registerCommand('lldb.setNextStatement', () => this.setNextStatement()));

        subscriptions.push(workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('lldb.displayFormat') ||
                event.affectsConfiguration('lldb.showDisassembly') ||
                event.affectsConfiguration('lldb.dereferencePointers')) {
                this.propagateDisplaySettings();
            }
        }));

        this.registerDisplaySettingCommand('lldb.showDisassembly', async (settings) => {
            settings.showDisassembly = <DisplaySettings['showDisassembly']>await window.showQuickPick(['always', 'auto', 'never']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDisassembly', async (settings) => {
            settings.showDisassembly = (settings.showDisassembly == 'auto') ? 'always' : 'auto';
        });
        this.registerDisplaySettingCommand('lldb.displayFormat', async (settings) => {
            settings.displayFormat = <DisplaySettings['displayFormat']>await window.showQuickPick(['auto', 'hex', 'decimal', 'binary']);
        });
        this.registerDisplaySettingCommand('lldb.toggleDerefPointers', async (settings) => {
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

    }

    registerDisplaySettingCommand(command: string, updater: (settings: DisplaySettings) => Promise<void>) {
        this.context.subscriptions.push(commands.registerCommand(command, async () => {
            let settings = this.getDisplaySettings();
            await updater(settings);
            this.setDisplaySettings(settings);
        }));
    }

    getDisplaySettings(): DisplaySettings {
        let folder = debug.activeDebugSession ? debug.activeDebugSession.workspaceFolder.uri : undefined;
        let config = workspace.getConfiguration('lldb', folder);
        let settings: DisplaySettings = {
            displayFormat: config.get('displayFormat'),
            showDisassembly: config.get('showDisassembly'),
            dereferencePointers: config.get('dereferencePointers'),
            containerSummary: true,
        };
        return settings;
    }

    async setDisplaySettings(settings: DisplaySettings) {
        let folder = debug.activeDebugSession ? debug.activeDebugSession.workspaceFolder.uri : undefined;
        let config = workspace.getConfiguration('lldb', folder);
        await config.update('displayFormat', settings.displayFormat);
        await config.update('showDisassembly', settings.showDisassembly);
        await config.update('dereferencePointers', settings.dereferencePointers);
    }

    async propagateDisplaySettings() {
        let settings = this.getDisplaySettings();

        this.status.text =
            `Format: ${settings.displayFormat}  ` +
            `Disasm: ${settings.showDisassembly}  ` +
            `Deref: ${settings.dereferencePointers ? 'on' : 'off'}`;

        if (debug.activeDebugSession && debug.activeDebugSession.type == 'lldb') {
            await debug.activeDebugSession.customRequest('displaySettings', settings);
        }
    }

    async setNextStatement() {
        try {
            const debugSession = debug.activeDebugSession;
            if (!debugSession) {
                throw new Error("There isn't an active CodeLLDB debug session.");
            }

            const debugType: string = debugSession.type;
            if (debugType !== "lldb") {
                throw new Error("There isn't an active CodeLLDB debug session.");
            }

            const currentEditor = window.activeTextEditor;
            if (!currentEditor) {
                throw new Error("There isn't an active source file.");
            }

            const position = currentEditor.selection.active;
            if (!position) {
                throw new Error("There isn't a current source position.");
            }

            const currentDocument = currentEditor.document;
            if (currentDocument.isDirty) {
                throw new Error("The current document has unsaved edits.");
            }

            const gotoTargetsArg : DebugProtocol.GotoTargetsArguments = {
                source: {
                    path: currentDocument.uri.fsPath
                },
                line: position.line + 1,
                column: position.character + 1
            };

            const gotoTargetsResponseBody = await debugSession.customRequest('gotoTargets', gotoTargetsArg);
            const targets: DebugProtocol.GotoTarget[] = gotoTargetsResponseBody.targets;
            if (targets.length === 0) {
                throw new Error(`No executable code is associated with line ${gotoTargetsArg.line}.`);
            }

            let selectedTarget = targets[0];

            if (targets.length > 1) {

                // If we have multiple possible targets, then let the user pick.
                const labelDict: { [key: string]: DebugProtocol.GotoTarget } = SetNextStatementHelpers.makeLabelsUnique(targets);
                const labels : string[] = targets.map((target) => target.label);

                const options: QuickPickOptions = {
                    matchOnDescription: true,
                    placeHolder: "Choose the specific location"
                };

                const selectedLabelName : string = await window.showQuickPick(labels, options);
                if (!selectedLabelName) {
                    return; // operation was cancelled
                }
                selectedTarget = labelDict[selectedLabelName];
            }

            const gotoArg : DebugProtocol.GotoArguments = {
                targetId : selectedTarget.id,
                threadId : 0
                };
                await debugSession.customRequest('goto', gotoArg);
        }
        catch (err) {
            window.showErrorMessage(`Unable to set the next statement. ${err}`);
        }
    }

    async changeDisplaySettings() {
        let settings = this.getDisplaySettings();
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

    async onActivate() {
        let pkg = extensions.getExtension('vadimcn.vscode-lldb').packageJSON;
        let currVersion = pkg.version;
        let lastVersion = this.context.globalState.get('lastLaunchedVersion');
        if (currVersion != lastVersion) {
            this.context.globalState.update('lastLaunchedVersion', currVersion);
            let choice = await window.showInformationMessage('CodeLLDB extension has been updated', 'What\'s new?');
            if (choice != null) {
                let changelog = path.join(this.context.extensionPath, 'CHANGELOG.md')
                let uri = Uri.parse(`file://${changelog}`);
                await commands.executeCommand('markdown.showPreview', uri, null, { locked: true });
            }
        }

        this.propagateDisplaySettings();
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
        output.appendLine(`configuration: ${inspect(launchConfig)}`);
        launchConfig._displaySettings = this.getDisplaySettings();
        return launchConfig;
    }

    async createDebugAdapterDescriptor(session: DebugSession, executable: DebugAdapterExecutable | undefined): Promise<DebugAdapterDescriptor> {
        let lldbConfig = workspace.getConfiguration('lldb', session.workspaceFolder ? session.workspaceFolder.uri : undefined);
        let adapterParams: any = this.getAdapterParameters(lldbConfig);
        if (session.configuration.sourceLanguages) {
            adapterParams.sourceLanguages = session.configuration.sourceLanguages;
            delete session.configuration.sourceLanguages;
        }

        try {
            let [adapter, port] = await this.startDebugAdapter(session.workspaceFolder, adapterParams);
            let descriptor = new DebugAdapterServer(port);
            return descriptor;
        } catch (err) {
            diagnostics.analyzeStartupError(err, this.context, output);
            throw err;
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
        let adapterEnv = config.get('adapterEnv', {});
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
            let executablePath = util.getConfigNoDefault(config, 'executable');
            let libraryPath = util.getConfigNoDefault(config, 'library');

            if (!executablePath && !libraryPath) { // Use bundled
                libraryPath = await adapter.findLibLLDB(path.join(this.context.extensionPath, 'lldb'));
            } else if (libraryPath) {
                libraryPath = await adapter.findLibLLDB(libraryPath)
            } else { // Infer from executablePath
                let dirs;
                let cachedDirs = this.context.workspaceState.get<any>('lldb_directories');
                if (!cachedDirs || cachedDirs.key != executablePath) {
                    dirs = await util.getLLDBDirectories(executablePath);
                    this.context.workspaceState.update('lldb_directories', { key: executablePath, value: dirs });
                } else {
                    dirs = cachedDirs.value;
                }
                libraryPath = await adapter.findLibLLDB(dirs.shlibDir);
            }

            if (!libraryPath)
                throw new Error('Could not locate liblldb');

            if (verboseLogging) {
                output.appendLine(`library: ${libraryPath}`);
                output.appendLine(`environment: ${inspect(adapterEnv)}`);
                output.appendLine(`params: ${inspect(params)}`);
            }

            adapterProcess = await adapter.startNative(
                this.context.extensionPath,
                libraryPath,
                adapterEnv,
                workspace.rootPath,
                params,
                verboseLogging);
        }
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


