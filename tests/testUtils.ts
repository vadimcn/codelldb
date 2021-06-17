import * as cp from 'child_process';
import * as stream from 'stream';
import * as adapter from 'extension/novsc/adapter';
import * as path from 'path';
import * as fs from 'fs';
import * as net from 'net';
import * as assert from 'assert';
import { inspect } from 'util';
import { WritableStream } from 'memory-streams';
import { Dict } from 'extension/novsc/commonTypes';
import { DebugClient } from 'vscode-debugadapter-testsupport';
import { DebugProtocol as dp } from 'vscode-debugprotocol';
import { type } from 'os';

let extensionRoot: string = null;
let testLog: stream.Writable = null;
let testDataLog: stream.Writable = null;
let adapterLog: stream.Writable = null;

export function initUtils(extensionRoot_: string) {
    const maxMessage = 1024 * 1024;
    extensionRoot = extensionRoot_;
    testLog = new WritableStream({ highWaterMark: maxMessage });
    //testDataLog = new WritableStream({ highWaterMark: maxMessage });
    adapterLog = new WritableStream({ highWaterMark: maxMessage });
}

export function findMarker(file: string, marker: string): number {
    let data = fs.readFileSync(file, 'utf8');
    let lines = data.split('\n');
    for (let i = 0; i < lines.length; ++i) {
        let pos = lines[i].indexOf(marker);
        if (pos >= 0) return i + 1;
    }
    throw Error('Marker not found');
}

export function charCode(ch: string): number {
    return ch.charCodeAt(0);
}

function asyncTimer(timeoutMillis: number): Promise<void> {
    return new Promise<void>((resolve) => setTimeout(resolve));
}

function withTimeout<T>(timeoutMillis: number, promise: Promise<T>): Promise<T> {
    let error = new Error('Timed out');
    return new Promise<T>((resolve, reject) => {
        let timer = setTimeout(() => {
            log('withTimeout: timed out');
            (<any>error).code = 'Timeout';
            reject(error);
        }, timeoutMillis);
        promise.then(result => {
            clearTimeout(timer);
            resolve(result);
        });
    });
}

function leftPad(s: string, p: string, n: number): string {
    if (s.length < n)
        s = p.repeat(n - s.length) + s;
    return s;
}

function timestamp(): string {
    let d = new Date();
    let hh = leftPad(d.getHours().toString(), '0', 2);
    let mm = leftPad(d.getMinutes().toString(), '0', 2);
    let ss = leftPad(d.getSeconds().toString(), '0', 2);
    let fff = leftPad(d.getMilliseconds().toString(), '0', 3);
    return `${hh}:${mm}:${ss}.${fff}`;
}

export function log(message: string) {
    testLog.write(`[${timestamp()}] ${message}\n`);
}

export function logWithStack(message: string) {
    log(message);
    let stack = Error().stack;
    let lines = stack.split('\n');
    for (let i = 2; i < lines.length; ++i)
        testLog.write(`${lines[i]}\n`);
}

export function dumpLogs(dest: stream.Writable) {
    dest.write('\n=== Test log ===\n');
    dest.write(testLog.toString());
    if (testDataLog) {
        dest.write('\n=== Received data log ===\n');
        dest.write(testDataLog.toString());
    }
    dest.write('\n=== Adapter log ===\n');
    dest.write(adapterLog.toString());
    dest.write('\n===================\n');
}

type ConfiguratorFn = () => Promise<any>;
type ValidatorFn = (v: dp.Variable) => boolean;

export class DebugTestSession extends DebugClient {
    adapter: cp.ChildProcess;
    port: number;

    static async start(): Promise<DebugTestSession> {
        let session = new DebugTestSession('', '', 'lldb');

        if (process.env.DEBUG_SERVER) {
            session.port = parseInt(process.env.DEBUG_SERVER)
        } else {
            let liblldb = await adapter.findLibLLDB(path.join(extensionRoot, 'lldb'));
            session.adapter = await adapter.start(liblldb, {
                extensionRoot: extensionRoot,
                extraEnv: { RUST_LOG: 'error,codelldb=debug' },
                adapterParameters: {},
                workDir: undefined,
                verboseLogging: true,
            });

            session.adapter.on('error', (err) => log(`Adapter error: ${err} `));
            session.adapter.on('exit', (code, signal) => {
                if (code != 0)
                    log(`Adapter exited with code ${code}, signal = ${signal} `);
            });

            session.adapter.stdout.pipe(adapterLog);
            session.adapter.stderr.pipe(adapterLog);
            session.port = await adapter.getDebugServerPort(session.adapter);
        }

        let logger = (event: dp.Event) => log(`Received event: ${inspect(event, { breakLength: Infinity })}`);
        session.addListener('breakpoint', logger);
        session.addListener('stopped', logger);
        session.addListener('continued', logger);
        session.addListener('exited', logger);
        await session.start(session.port);

        if (testDataLog) {
            let socket = <net.Socket>((<any>session)._socket);
            socket.on('data', buffer => {
                testDataLog.write(`[${timestamp()}] --> ${buffer} \n`)
            });
        }
        return session;
    }

    async terminate() {
        log('Stopping adapter.');
        await super.stop();
        // Check that adapter process exits
        let adapterExit = new Promise((resolve) => this.adapter.on('exit', resolve));
        await withTimeout(3000, adapterExit);
    }

    async launch(launchArgs: any, configurator: ConfiguratorFn = null): Promise<dp.LaunchResponse> {
        launchArgs.terminal = 'console';
        let waitForInitialized = this.waitForEvent('initialized');
        await this.initializeRequest()
        let launchResp = this.launchRequest(launchArgs);
        await waitForInitialized;
        if (configurator)
            await configurator();
        this.configurationDoneRequest();
        return launchResp;
    }

    async attach(attachArgs: any, configurator: ConfiguratorFn = null): Promise<dp.AttachResponse> {
        let waitForInitialized = this.waitForEvent('initialized');
        await this.initializeRequest()
        let attachResp = this.attachRequest(attachArgs);
        await waitForInitialized;
        if (configurator)
            await configurator();
        this.configurationDoneRequest();
        return attachResp;
    }

    async setBreakpoint(file: string, line: number, condition?: string): Promise<dp.SetBreakpointsResponse> {
        let breakpointResp = await this.setBreakpointsRequest({
            source: { path: file },
            breakpoints: [{ line: line, column: 0, condition: condition }],
        });
        let bp = breakpointResp.body.breakpoints[0];
        log(`Received setBreakpoint response: ${inspect(bp, { breakLength: Infinity })}`);
        return breakpointResp;
    }

    async setFnBreakpoint(name: string, condition?: string): Promise<dp.SetFunctionBreakpointsResponse> {
        let breakpointResp = await this.setFunctionBreakpointsRequest({
            breakpoints: [{ name: name, condition: condition }]
        });
        return breakpointResp;
    }

    async verifyLocation(threadId: number, file: string, line: number) {
        let stackResp = await this.stackTraceRequest({ threadId: threadId });
        let topFrame = stackResp.body.stackFrames[0];
        assert.equal(topFrame.line, line);
    }

    async readVariables(variablesReference: number): Promise<Dict<dp.Variable>> {
        logWithStack('Awaiting variables');
        let response = await this.variablesRequest({ variablesReference: variablesReference });
        let vars: Dict<dp.Variable> = {};
        for (let v of response.body.variables) {
            vars[v.name] = v;
        }
        return vars;
    }

    async compareVariables(
        vars: number | Dict<dp.Variable>,
        expected: Dict<string | number | boolean | ValidatorFn | Dict<any>>,
        prefix: string = ''
    ) {
        if (typeof vars == 'number') {
            assert.notEqual(vars, 0, 'Expected non-zero.');
            vars = await this.readVariables(vars);
        }

        for (let key of Object.keys(expected)) {
            if (key == '$')
                continue; // Summary will have been checked by the caller.

            let keyPath = prefix.length > 0 ? prefix + '.' + key : key;
            let expectedValue = expected[key];
            let variable = vars[key];
            assert.notEqual(variable, undefined, 'Did not find variable "' + keyPath + '"');

            if (typeof expectedValue == 'object') {
                let summary = expectedValue['$'];
                if (summary != undefined) {
                    this.compareToExpected(variable, summary, keyPath);
                }
                logWithStack('Awaiting compareVariables');
                await this.compareVariables(variable.variablesReference, expectedValue, keyPath);
            } else {
                this.compareToExpected(variable, expectedValue, keyPath);
            }
        }
    }

    compareToExpected(variable: dp.Variable, expectedValue: string | number | boolean | ValidatorFn, keyPath: string) {
        if (typeof expectedValue == 'string') {
            assert.equal(variable.value, expectedValue,
                `"${keyPath}": expected: "${expectedValue}", actual: "${variable.value}"`);
        } else if (typeof expectedValue == 'boolean') {
            let boolValue = variable.value == 'true' ? true : variable.value == 'false' ? false : null;
            assert.equal(boolValue, expectedValue,
                `"${keyPath}": expected: "${expectedValue}", actual: "${variable.value}"`);
        } else if (typeof expectedValue == 'number') {
            if (Number.isSafeInteger(expectedValue)) {
                let numValue = parseInt(variable.value);
                assert.equal(numValue, expectedValue,
                    `"${keyPath}": expected: "${expectedValue}", actual: "${variable.value}"`);
            } else { // approximate comparison for floats
                let numValue = parseFloat(variable.value);
                let delta = Math.abs(numValue - expectedValue);
                assert.ok(delta < 1e-6 || delta / expectedValue < 1e-6,
                    `"${keyPath}": expected: ${expectedValue}, actual: ${numValue} `);
            }
        } else if (typeof expectedValue == 'function') {
            assert.ok(expectedValue(variable),
                `"${keyPath}": validator returned false`);
        } else {
            assert.ok(false, 'Unreachable');
        }
    }

    waitForStopEvent(): Promise<dp.StoppedEvent> {
        let session = this;
        logWithStack('Listening for stop event');
        return new Promise<dp.StoppedEvent>(resolve => {
            let handler = (event: dp.StoppedEvent) => {
                if (event.body.reason != 'initial') {
                    session.removeListener('stopped', handler);
                    log('Resolving stop event')
                    resolve(event);
                } else {
                    log('Ignored "initial" event');
                }
            };
            session.addListener('stopped', handler);
        });
    }

    async launchAndWaitForStop(launchArgs: any, configurator: ConfiguratorFn = null): Promise<dp.StoppedEvent> {
        let waitForStopAsync = this.waitForStopEvent();
        logWithStack('Awaiting launch');
        await this.launch(launchArgs, configurator);
        logWithStack('Awaiting stop');
        let stoppedEvent = await waitForStopAsync;
        return <dp.StoppedEvent>stoppedEvent;
    }

    async launchStopAndGetVars(launchArgs: any, bpFile: string, bpLine: number): Promise<Dict<dp.Variable>> {
        let stoppedEvent = await this.launchAndWaitForStop(launchArgs,
            async () => {
                await this.setBreakpoint(bpFile, bpLine);
            });
        await this.verifyLocation(stoppedEvent.body.threadId, bpFile, bpLine);
        let frames = await this.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 0, levels: 1 });
        let scopes = await this.scopesRequest({ frameId: frames.body.stackFrames[0].id });
        let localVars = await this.readVariables(scopes.body.scopes[0].variablesReference);
        return localVars;
    }

    async getTopFrameId(threadId: number): Promise<number> {
        logWithStack('Awaiting stack trace');
        let frames = await this.stackTraceRequest({ threadId: threadId, startFrame: 0, levels: 1 });
        return frames.body.stackFrames[0].id;
    }

    async getFrameLocalsRef(frameId: number): Promise<number> {
        logWithStack('Awaiting scopes');
        let scopes = await this.scopesRequest({ frameId: frameId });
        let localsRef = scopes.body.scopes[0].variablesReference;
        return localsRef;
    }
}

export function char(ch: string): ValidatorFn {
    assert.equal(ch.length, 1);
    return v => parseInt(v.value) == ch.charCodeAt(0) || v.value == `'${ch}'`;
}
