import { workspace, window, WorkspaceFolder, ConfigurationTarget } from 'vscode';
import * as path from 'node:path'
import * as adapter from './novsc/adapter';
import { DebugClient } from './novsc/debugClient';
import * as async from './novsc/async';
import { output } from './logging';
import * as util from './configUtils';
import { getExtensionConfig } from './main';
import { AddressInfo } from 'node:net';


export async function alternateBackend(extensionPath: string) {
    let box = window.createInputBox();
    box.prompt = 'Enter file name of the LLDB instance you\'d like to use. ';
    box.onDidAccept(async () => {
        try {
            let dirs = await util.getLLDBDirectories(box.value);
            if (dirs) {
                let lldbDir = path.resolve(path.join(dirs.supportExeDir, '..'));
                let libraryPath = await adapter.findLibLLDB(lldbDir);
                let serverPath;
                if (process.platform == 'linux') {
                    serverPath = await adapter.findFileByPattern(dirs.supportExeDir, /lldb-server(-.*)?/);
                    if (serverPath) {
                        let stats = await async.fs.stat(serverPath).catch(_ => null);
                        if (!stats || (stats.mode & 1) == 0)
                            serverPath = undefined;
                    }
                }
                if (libraryPath) {
                    let startOptions: adapter.AdapterStartOptions = {
                        extensionPath: extensionPath,
                        liblldb: libraryPath,
                        lldbServer: serverPath,
                        connect: true,
                        verboseLogging: true,
                    };
                    let testSucceeded = await selfTest(startOptions).catch(_ => false);

                    let message = `Located liblldb at: ${libraryPath}`;
                    if (serverPath)
                        message += `\r\nand lldb server at: ${serverPath}`;
                    else if (process.platform == 'linux')
                        message += '\r\nbut did not find lldb server.';

                    let choice;
                    if (testSucceeded) {
                        message += '\r\n\nDo you want to configure alternate backend for the current workspace?';
                        choice = await window.showInformationMessage(message, { modal: true }, 'Yes');
                    } else {
                        message += '\r\n\nHowever, the debug adapter self-test has FAILED!';
                        message += '\r\n\nDo you still want to configure alternate backend for the current workspace? (not recommended)';
                        output.show();
                        choice = await window.showErrorMessage(message, { modal: true }, 'Yes');
                    }

                    if (choice == 'Yes') {
                        box.hide();
                        let lldbConfig = getExtensionConfig();
                        lldbConfig.update('library', libraryPath, ConfigurationTarget.Workspace);
                        if (serverPath)
                            lldbConfig.update('server', serverPath, ConfigurationTarget.Workspace);
                    } else {
                        box.show();
                    }
                }
            }
        } catch (err: any) {
            let message = (err?.code == 'ENOENT') ? `could not find "${err.path}".` : err.message;
            await window.showErrorMessage(`Failed to query LLDB for library location: ${message}`, { modal: true });
            box.show();
        }
    });
    box.show();
}

export async function selfTest(startOptions: adapter.AdapterStartOptions, timeout: number = 5000): Promise<boolean> {
    try {
        let server = new async.net.Server();
        await server.listen({ port: 0, backlog: 1 });
        startOptions.port = (server.address() as AddressInfo).port;
        let adapterProcess = await adapter.start(startOptions);
        util.logProcessOutput(adapterProcess, output);
        setTimeout(() => adapterProcess.kill(), timeout);

        let connection = await server.accept();
        server.close(); // No new connections

        let dc = new DebugClient('lldb');
        dc.connect(connection, connection);
        await Promise.all([
            dc.configurationSequence(),
            dc.launch({ program: path.join(startOptions.extensionPath, 'bin', 'codelldb-launch') })
        ]);
        await dc.disconnectRequest();
        connection.end();

        let exitCode = await adapterProcess.exit;
        return exitCode == 0;
    } catch (err) {
        return false;
    }
}

export async function commandPrompt(extensionRoot: string) {
    let lldb = process.platform != 'win32' ? 'lldb' : 'lldb.exe';
    let lldbPath = path.join(extensionRoot, 'lldb', 'bin', lldb);
    let consolePath = path.join(extensionRoot, 'adapter', 'scripts', 'console.py');
    let folder = workspace.workspaceFolders?.[0];
    let config = getExtensionConfig(folder);
    let env = adapter.getAdapterEnv(config.get('adapterEnv', {}));

    let terminal = window.createTerminal({
        name: 'LLDB Command Prompt',
        shellPath: lldbPath,
        shellArgs: ['--no-lldbinit', '--one-line-before-file', 'command script import ' + consolePath],
        cwd: folder?.uri,
        env: env,
        strictEnv: true
    });
    terminal.show()
}
