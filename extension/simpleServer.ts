import * as net from 'net';
import { EventEmitter } from 'vscode';


export class SimpleServer {
    inner: net.Server;
    errorEmitter = new EventEmitter<Error>();
    readonly onError = this.errorEmitter.event;

    constructor(options: net.SocketConstructorOpts) {
        this.inner = net.createServer(options);
        this.inner.on('error', err => this.errorEmitter.fire(err));
        this.inner.on('connection', socket => {
            let request = '';
            socket.on('data', chunk => request += chunk);
            socket.on('end', () => {
                let response = this.processRequest(request);
                if (response instanceof Promise) {
                    response.then(value => socket.end(value));
                } else {
                    socket.end(response);
                }
            });
        });
    }

    public processRequest: (request: string) => (string | Promise<string>);

    public async listen(options: net.ListenOptions) {
        return new Promise<net.AddressInfo | string>(resolve =>
            this.inner.listen(options, () => resolve(this.inner.address()))
        );
    }

    public close() {
        this.inner.close();
    }
}
