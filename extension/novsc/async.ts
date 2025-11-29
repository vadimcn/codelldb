import * as _fs from 'fs';
import * as _cp from 'child_process';
import * as _http from 'http';
import * as _https from 'https';
import * as _net from 'net';
import { promisify } from 'util';

export namespace fs {
    export const readdir = promisify(_fs.readdir);
    export const readFile = promisify(_fs.readFile);
    export const writeFile = promisify(_fs.writeFile);
    export const exists = promisify(_fs.exists);
    export const stat = promisify(_fs.stat);
    export const copyFile = promisify(_fs.copyFile);
    export const unlink = promisify(_fs.unlink);
    export const mkdir = promisify(_fs.mkdir);
}

export namespace cp {
    export const execFile = promisify(_cp.execFile);

    export type ChildProcess = _cp.ChildProcess & { exit: Promise<number> };

    export function spawn(command: string, args?: readonly string[], options?: _cp.SpawnOptions): ChildProcess {
        let subproc = _cp.spawn(command, args ?? [], options ?? {}) as ChildProcess;

        // Promise that resolves on child process exit (or failure to spawn)
        subproc.exit = new Promise((resolve, reject) => {
            subproc.on('error', err => {
                if (!subproc.pid) // Otherwise it isn't a spawning error
                    reject(err);
            });
            subproc.on('exit', (code, _signal) => {
                resolve(code ?? -1);
            });
        });
        return subproc;
    }
}

export namespace https {
    export function get(url: string | URL): Promise<_http.IncomingMessage> {
        return new Promise<_http.IncomingMessage>((resolve, reject) => {
            let request = _https.get(url, resolve);
            request.on('error', reject);
        });
    }
}

export namespace net {
    export function createConnection(options: _net.NetConnectOpts): Promise<_net.Socket> {
        return new Promise((resolve, reject) => {
            let socket = _net.createConnection(options);
            socket.on('error', reject);
            socket.on('connect', () => resolve(socket));
        });
    }
    export class Server {
        private inner: _net.Server = _net.createServer();

        async listen(options?: _net.ListenOptions): Promise<void> {
            return new Promise((resolve) =>
                this.inner.listen(options, resolve)
            );
        }

        address(): _net.AddressInfo | string | null {
            return this.inner.address();
        }

        async accept(): Promise<_net.Socket> {
            return new Promise<_net.Socket>(resolve =>
                this.inner.on('connection', socket => resolve(socket))
            );
        }

        async close(): Promise<void> {
            return new Promise<void>((resolve, reject) =>
                this.inner.close(err => { if (err) resolve(); else reject(err) })
            )
        }
    }
}

export async function sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}
