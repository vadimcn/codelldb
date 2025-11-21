import * as vscode from 'vscode';
import * as assert from 'node:assert';
import * as cp from 'node:child_process';
import * as path from 'node:path';
import { RpcLaunchServer } from 'extension/externalLaunch';
import { AddressInfo } from 'node:net';
import { inspect } from 'util';

suite('Extension Tests', () => {
    let logger: Logger;

    suiteSetup(function () {
        logger = new Logger();
        vscode.debug.registerDebugAdapterTrackerFactory('lldb', logger);
    });

    teardown(function () {
        if (this.currentTest?.state == 'failed') {
            for (let line of logger.lines) {
                console.error(line);
            }
        }
        logger.clear();
    });

    test('Cargo build launch', async () => {
        let success = await vscode.debug.startDebugging(vscode.workspace.workspaceFolders[0], {
            type: 'lldb',
            name: 'test',
            request: 'launch',
            cargo: ['build', '--bin', 'rust-debuggee', '--manifest-path=${workspaceFolder}/rust/Cargo.toml']
        });
        assert.ok(success);
    });

    test('Cargo run launch', async () => {
        let success = await vscode.debug.startDebugging(vscode.workspace.workspaceFolders[0], {
            type: 'lldb',
            name: 'test',
            request: 'launch',
            cargo: ['run', '--bin', 'rust-debuggee', '--manifest-path=${workspaceFolder}/rust/Cargo.toml']
        });
        assert.ok(success);
    });

    test('RPC launch', async () => {
        let rpcServer = new RpcLaunchServer({ token: 'secret' });
        let addrinfo = await rpcServer.listen({ host: '127.0.0.1', port: 0 }) as AddressInfo;

        let ext = vscode.extensions.getExtension('vadimcn.vscode-lldb');
        let launcher = path.join(ext.extensionPath, 'bin', 'codelldb-launch');
        let proc = cp.spawn(launcher, [
            `--connect=${addrinfo.address}:${addrinfo.port}`,
            '--config={ token: secret }',
            'debuggee/debuggee',
        ], { cwd: ext.extensionPath, stdio: 'pipe', env: { ...process.env, RUST_LOG: 'debug' }});
        proc.stdout.on('data', buf => console.log(buf.toString()));
        proc.stderr.on('data', buf => console.error(buf.toString()));

        await new Promise<void>((resolve, reject) => {
            proc.on('error', err => reject(err));
            proc.on('exit', (code, signal) => {
                if (code == 0) {
                    resolve();
                } else {
                    reject(Error(`Launcher exited with code: ${code}, signal: ${signal}`));
                }
            });
        });
        rpcServer.close();
    });
});


class Logger implements vscode.DebugAdapterTrackerFactory, vscode.DebugAdapterTracker {
    lines: string[] = [];
    inspectOptions = { compact: true, breakLength: Infinity, colors: true };

    clear() {
        this.lines.splice(0);
    }
    createDebugAdapterTracker(session: vscode.DebugSession): vscode.ProviderResult<vscode.DebugAdapterTracker> {
        return this;
    }
    onWillReceiveMessage(message: any): void {
        this.lines.push(`--> ${inspect(message, this.inspectOptions)}`);
    }
    onDidSendMessage(message: any): void {
        this.lines.push(`<-- ${inspect(message, this.inspectOptions)}`);
    }
    onError?(error: Error): void {
        this.lines.push(`Adapter comms error: ${error}`);
    }
}
