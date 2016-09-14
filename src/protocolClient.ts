/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import stream = require('stream');
import * as ee from 'events';
import {DebugProtocol} from 'vscode-debugprotocol';

export class ProtocolClient extends ee.EventEmitter {

	private static TWO_CRLF = '\r\n\r\n';

	private outputStream: stream.Writable;
	private sequence: number;
	private pendingRequests = new Map<number, (e: DebugProtocol.Response) => void>();
	private rawData = new Buffer(0);
	private contentLength: number;

	constructor() {
		super();
		this.sequence = 1;
		this.contentLength = -1;
	}

	protected connect(readable: stream.Readable, writable: stream.Writable): void {

		this.outputStream = writable;

		readable.on('data', (data: Buffer) => {
			this.handleData(data);
		});
	}

	public send(command: string, args?: any): Promise<DebugProtocol.Response> {

		return new Promise((completeDispatch, errorDispatch) => {
			this.doSend(command, args, (result: DebugProtocol.Response) => {
				if (result.success) {
					completeDispatch(result);
				} else {
					errorDispatch(new Error(result.message));
				}
			});
		});
	}

	private doSend(command: string, args: any, clb: (result: DebugProtocol.Response) => void): void {

		const request: DebugProtocol.Request = {
			type: 'request',
			seq: this.sequence++,
			command: command
		};
		if (args && Object.keys(args).length > 0) {
			request.arguments = args;
		}

		// store callback for this request
		this.pendingRequests.set(request.seq, clb);

		const json = JSON.stringify(request);
		this.outputStream.write(`Content-Length: ${Buffer.byteLength(json, 'utf8')}\r\n\r\n${json}`, 'utf8');
	}

	private handleData(data: Buffer): void {

		this.rawData = Buffer.concat([this.rawData, data]);

		while (true) {
			if (this.contentLength >= 0) {
				if (this.rawData.length >= this.contentLength) {
					const message = this.rawData.toString('utf8', 0, this.contentLength);
					this.rawData = this.rawData.slice(this.contentLength);
					this.contentLength = -1;
					if (message.length > 0) {
						this.dispatch(message);
					}
					continue;	// there may be more complete messages to process
				}
			} else {
				const idx = this.rawData.indexOf(ProtocolClient.TWO_CRLF);
				if (idx !== -1) {
					const header = this.rawData.toString('utf8', 0, idx);
					const lines = header.split('\r\n');
					for (let i = 0; i < lines.length; i++) {
						const pair = lines[i].split(/: +/);
						if (pair[0] === 'Content-Length') {
							this.contentLength = +pair[1];
						}
					}
					this.rawData = this.rawData.slice(idx + ProtocolClient.TWO_CRLF.length);
					continue;
				}
			}
			break;
		}
	}

	private dispatch(body: string): void {

		const rawData = JSON.parse(body);

		if (typeof rawData.event !== 'undefined') {
			const event = <DebugProtocol.Event> rawData;
			this.emit(event.event, event);
		} else {
			const response = <DebugProtocol.Response> rawData;
			const clb = this.pendingRequests.get(response.request_seq);
			if (clb) {
				this.pendingRequests.delete(response.request_seq);
				clb(response);
			}
		}
	}
}
