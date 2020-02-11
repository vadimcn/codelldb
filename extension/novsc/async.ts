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
}

export namespace cp {
    export const execFile = promisify(_cp.execFile);
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
}

export async function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}
