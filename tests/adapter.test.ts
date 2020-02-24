import { suite, test } from 'mocha';
import * as assert from 'assert';
import * as path from 'path';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as net from 'net';
import * as stream from 'stream';
import { inspect } from 'util';
import { DebugClient } from 'vscode-debugadapter-testsupport';
import { DebugProtocol as dp } from 'vscode-debugprotocol';
import { WritableStream } from 'memory-streams';

import { Dict } from 'extension/novsc/commonTypes';
import * as adapter from 'extension/novsc/adapter';

const triple = process.env.TARGET_TRIPLE || '';
const buildDir = process.env.BUILD_DIR || path.dirname(__dirname); // tests are located in $buildDir/tests
const sourceDir = process.env.SOURCE_DIR || path.dirname(buildDir); // assume $sourceDir is the parent of $buildDir
const dumpLogsWhen = (process.env.DUMP_LOGS || 'onerror').toLowerCase();

const extensionRoot = buildDir;

let debuggeeDir = path.join(buildDir, 'debuggee');
if (triple.endsWith('pc-windows-gnu'))
    debuggeeDir = path.join(buildDir, 'debuggee-gnu');
else if (triple.endsWith('pc-windows-msvc'))
    debuggeeDir = path.join(buildDir, 'debuggee-msvc');

const debuggee = path.join(debuggeeDir, 'debuggee');
const debuggeeSource = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'debuggee.cpp'));
const debuggeeHeader = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'dir1', 'debuggee.h'));
const debuggeeDenorm = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'denorm_path.cpp'));
const debuggeeRemote1 = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'remote1', 'remote_path.cpp'));
const debuggeeRemote2 = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'remote2', 'remote_path.cpp'));
const debuggeeRelative = path.normalize(path.join(sourceDir, 'debuggee', 'cpp', 'relative_path.cpp'));

const rustDebuggee = path.join(debuggeeDir, 'rust-debuggee');
const rustDebuggeeSource = path.normalize(path.join(sourceDir, 'debuggee', 'rust', 'types.rs'));

var testLog: stream.Writable;
var testDataLog: stream.Writable;
var adapterLog: stream.Writable;

generateSuite(triple);

function generateSuite(triple: string) {
    suite(`adapter:${triple}`, () => {

        setup(function () {
            const maxMessage = 1024 * 1024;
            testLog = new WritableStream({ highWaterMark: maxMessage });
            //testDataLog = new WritableStream({ highWaterMark: maxMessage });
            adapterLog = new WritableStream({ highWaterMark: maxMessage });
        });

        teardown(async function () {
            if (dumpLogsWhen != 'never' && (this.currentTest.state == 'failed' || dumpLogsWhen == 'always'))
                dumpLogs(process.stderr);
        });

        suite('Basic', () => {

            test('check python', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                await ds.launch({ name: 'check python', custom: true });
                let result = await ds.evaluateRequest({
                    expression: 'script import lldb; print(lldb.debugger.GetVersionString())',
                    context: 'repl'
                });
                assert.ok(result.body.result.startsWith('lldb version'));
                assert.ok(result.body.result.indexOf('rust-enabled') >= 0);
                await ds.terminate();
            });

            test('run program to the end', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let terminatedAsync = ds.waitForEvent('terminated');
                await ds.launch({ name: 'run program to the end', program: debuggee });
                await terminatedAsync;
                await ds.terminate();
            });

            test('run program with modified environment', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let waitExitedAsync = ds.waitForEvent('exited');
                await ds.launch({
                    name: 'run program with modified environment',
                    env: { 'FOO': 'bar' },
                    program: debuggee,
                    args: ['check_env', 'FOO', 'bar'],
                });
                let exitedEvent = await waitExitedAsync;
                // debuggee shall return 1 if env[argv[2]] == argv[3]
                assert.equal(exitedEvent.body.exitCode, 1);
                await ds.terminate();
            });

            test('stop on entry', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let stopAsync = ds.waitForEvent('stopped');
                await ds.launch({ name: 'stop on entry', program: debuggee, args: ['inf_loop'], stopOnEntry: true });
                log('Waiting for stop');
                await stopAsync;
                log('Terminating');
                await ds.terminate();
            });

            test('stop on a breakpoint (basic)', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLineSource = findMarker(debuggeeSource, '#BP1');
                let setBreakpointAsyncSource = ds.setBreakpoint(debuggeeSource, bpLineSource);

                let waitForExitAsync = ds.waitForEvent('exited');
                let waitForStopAsync = ds.waitForStopEvent();

                await ds.launch({ name: 'stop on a breakpoint (basic)', program: debuggee, cwd: path.dirname(debuggee) });
                await setBreakpointAsyncSource;

                log('Wait for stop');
                let stopEvent = await waitForStopAsync;
                await ds.verifyLocation(stopEvent.body.threadId, debuggeeSource, bpLineSource);

                log('Continue');
                await ds.continueRequest({ threadId: 0 });
                log('Wait for exit');
                await waitForExitAsync;
                await ds.terminate();
            });

            test('stop on a breakpoint (same file name)', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLineSource = findMarker(debuggeeSource, '#BP1');
                let bpLineHeader = findMarker(debuggeeHeader, '#BPH1');
                let setBreakpointAsyncSource = ds.setBreakpoint(debuggeeSource, bpLineSource);
                let setBreakpointAsyncHeader = ds.setBreakpoint(debuggeeHeader, bpLineHeader);

                let waitForExitAsync = ds.waitForEvent('exited');
                let waitForStopAsync = ds.waitForStopEvent();

                // let testcase = triple.endsWith('windows-gnu') ?
                //     'header_nodylib' : // FIXME: loading dylib triggers a weird access violation on windows-gnu
                //     'header';
                let testcase = 'header_nodylib';

                await ds.launch({ name: 'stop on a breakpoint (same file name)', program: debuggee, args: [testcase], cwd: path.dirname(debuggee) });
                log('Set breakpoint 1');
                await setBreakpointAsyncSource;
                log('Set breakpoint 2');
                await setBreakpointAsyncHeader;

                log('Wait for stop 1');
                let stopEvent = await waitForStopAsync;
                await ds.verifyLocation(stopEvent.body.threadId, debuggeeSource, bpLineSource);

                let waitForStopAsync2 = ds.waitForStopEvent();
                log('Continue 1');
                await ds.continueRequest({ threadId: 0 });
                log('Wait for stop 2');
                let stopEvent2 = await waitForStopAsync2;
                await ds.verifyLocation(stopEvent2.body.threadId, debuggeeHeader, bpLineHeader);

                log('Continue 2');
                await ds.continueRequest({ threadId: 0 });
                log('Wait for exit');
                await waitForExitAsync;
                await ds.terminate();
            });

            test('path mapping', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let bpLineDenorm = findMarker(debuggeeDenorm, '#BP1');
                let bpLineRemote1 = findMarker(debuggeeRemote1, '#BP1');
                let bpLineRemote2 = findMarker(debuggeeRemote2, '#BP1')
                let bpLineRelative = findMarker(debuggeeRelative, '#BP1')
                let setBreakpointAsyncDenorm = ds.setBreakpoint(debuggeeDenorm, bpLineDenorm);
                let setBreakpointAsyncRemote1 = ds.setBreakpoint(debuggeeRemote1, bpLineRemote1);
                let setBreakpointAsyncRemote2 = ds.setBreakpoint(debuggeeRemote2, bpLineRemote2);
                let setBreakpointAsyncRelative = ds.setBreakpoint(debuggeeRelative, bpLineRelative);

                let waitForExitAsync = ds.waitForEvent('exited');
                let waitForStopAsync = ds.waitForStopEvent();

                // On Windows, LLDB adds current drive letter to drive-less paths.
                let drive = process.platform == 'win32' ? 'C:' : '';
                await ds.launch({
                    name: 'stop on a breakpoint (mapt remapping)', program: debuggee, args: ['weird_path'], cwd: path.dirname(debuggee),
                    sourceMap: {
                        [`${drive}/remote1`]: path.join(sourceDir, 'debuggee', 'cpp', 'remote1'),
                        [`${drive}/remote2`]: path.join(sourceDir, 'debuggee', 'cpp', 'remote2'),
                        ['.']: path.join(sourceDir, 'debuggee'),
                    },
                    relativePathBase: path.join(sourceDir, 'debuggee'),
                    preRunCommands: [
                        `set show target.source-map`
                    ]
                });

                // Wait for breakpoints to be resolved and verify locations.
                log('Set breakpoint 1');
                await setBreakpointAsyncDenorm;
                log('Set breakpoint 2');
                await setBreakpointAsyncRemote1;
                log('Set breakpoint 3');
                await setBreakpointAsyncRemote2;
                log('Set breakpoint 4');
                await setBreakpointAsyncRelative;

                // Wait for stops and verify stop locations.
                log('Wait for stop 1');
                let stopEvent1 = await waitForStopAsync;
                await ds.verifyLocation(stopEvent1.body.threadId, debuggeeDenorm, bpLineDenorm);

                let waitForStopAsync2 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: 0 });
                log('Wait for stop 2');
                let stopEvent2 = await waitForStopAsync2;
                await ds.verifyLocation(stopEvent2.body.threadId, debuggeeRemote1, bpLineRemote1);

                let waitForStopAsync3 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: 0 });
                log('Wait for stop 3');
                let stopEvent3 = await waitForStopAsync3;
                await ds.verifyLocation(stopEvent3.body.threadId, debuggeeRemote2, bpLineRemote2);

                let waitForStopAsync4 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: 0 });
                log('Wait for stop 4');
                let stopEvent4 = await waitForStopAsync4;
                await ds.verifyLocation(stopEvent4.body.threadId, debuggeeRelative, bpLineRelative);

                await ds.continueRequest({ threadId: 0 });
                log('Wait for exit');
                await waitForExitAsync;
                await ds.terminate();
            });

            test('page stack', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP2');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine);
                let waitForStopAsync = ds.waitForStopEvent();
                await ds.launch({ name: 'page stack', program: debuggee, args: ['deepstack'] });
                log('Wait for setBreakpoint');
                await setBreakpointAsync;
                log('Wait for stop');
                let stoppedEvent = await waitForStopAsync;
                let response2 = await ds.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 20, levels: 10 });
                assert.equal(response2.body.stackFrames.length, 10)
                let response3 = await ds.scopesRequest({ frameId: response2.body.stackFrames[0].id });
                let response4 = await ds.variablesRequest({ variablesReference: response3.body.scopes[0].variablesReference });
                assert.equal(response4.body.variables[0].name, 'levelsToGo');
                assert.equal(response4.body.variables[0].value, '20');
                await ds.terminate();
            });

            test('variables', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine);
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'variables', program: debuggee, args: ['vars'] });
                await ds.verifyLocation(stoppedEvent.body.threadId, debuggeeSource, bpLine);
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);

                await ds.compareVariables(localsRef, {
                    a: 30,
                    b: 40,
                    pi: 3.141592,
                    array_int: {
                        '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5, '[5]': 6, '[6]': 7, '[7]': 8, '[8]': 9, '[9]': 10,
                    },

                    s1: {
                        $: "{a:1, b:'a', c:3}",
                        a: 1, b: "'a'", c: 3
                    },
                    s_ptr: { a: 1, b: "'a'", c: 3 },
                    s_ptr_ptr: v => v.value.startsWith('{0x'),

                    s2: { a: 10, b: "'b'", c: 999 },
                    cstr: '"The quick brown fox"',
                    wcstr: 'L"The quick brown fox"',
                    str1: '"The quick brown fox"',
                    str_ptr: '"The quick brown fox"',
                    str_ref: '"The quick brown fox"',
                    empty_str: '""',
                    wstr1: 'L"Превед йожэг!"',
                    wstr2: 'L"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!"',

                    invalid_utf8: '"ABC\uFFFD\\x01\uFFFDXYZ',
                    anon_union: {
                        '': { x: 4, y: 4 }
                    },

                    null_s_ptr: '<null>',
                    null_s_ptr_ptr: v => v.value.startsWith('{0x'),
                    invalid_s_ptr: '<invalid address>',
                    void_ptr: v => v.value.startsWith('0x'),
                });

                let response1 = await ds.evaluateRequest({
                    expression: 'vec_int', context: 'watch', frameId: frameId
                });
                if (process.platform != 'win32') {
                    await ds.compareVariables(response1.body.variablesReference, {
                        '[0]': { '[0]': 0, '[1]': 0, '[2]': 0, '[3]': 0, '[4]': 0 },
                        '[9]': { '[0]': 0, '[1]': 0, '[2]': 0, '[3]': 0, '[4]': 0 },
                        '[raw]': _ => true
                    });
                }

                // Read a class-qualified static.
                let response2 = await ds.evaluateRequest({
                    expression: 'Klazz::m1', context: 'watch', frameId: frameId
                });
                assert.equal(response2.body.result, '42');

                // Check format-as-array.
                let response3 = await ds.evaluateRequest({
                    expression: 'array_int_ptr,[10]', context: 'watch', frameId: frameId
                });
                await ds.compareVariables(response3.body.variablesReference, {
                    '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5, '[5]': 6, '[6]': 7, '[7]': 8, '[8]': 9, '[9]': 10,
                });

                // Set a variable and check that it has actually changed.
                await ds.send('setVariable', { variablesReference: localsRef, name: 'a', value: '100' });
                await ds.compareVariables(localsRef, { a: 100 });
                await ds.terminate();
            });

            test('expressions', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine);
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'expressions', program: debuggee, args: ['vars'] });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);

                log('Waiting a+b');
                let response1 = await ds.evaluateRequest({ expression: "a+b", frameId: frameId, context: "watch" });
                assert.equal(response1.body.result, "70");

                log('Waiting /py...');
                let response2 = await ds.evaluateRequest({ expression: "/py sum([int(x) for x in $array_int])", frameId: frameId, context: "watch" });
                assert.equal(response2.body.result, "55"); // sum(1..10)

                // let response3 = await ds.evaluateRequest({ expression: "/nat 2+2", frameId: frameId, context: "watch" });
                // assert.ok(response3.body.result.endsWith("4")); // "(int) $0 = 70"

                for (let i = 1; i < 10; ++i) {
                    let waitForStopAsync = ds.waitForStopEvent();
                    log(`${i}: continue`);
                    await ds.continueRequest({ threadId: 0 });

                    log(`${i}: waiting for stop`);
                    let stoppedEvent = await waitForStopAsync;
                    let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);

                    log(`${i}: evaluate`);
                    let response1 = await ds.evaluateRequest({ expression: 's1.d', frameId: frameId, context: 'watch' });
                    let response2 = await ds.evaluateRequest({ expression: 's2.d', frameId: frameId, context: 'watch' });

                    log(`${i}: compareVariables`);
                    await ds.compareVariables(response1.body.variablesReference, { '[0]': i, '[1]': i, '[2]': i, '[3]': i });
                    await ds.compareVariables(response2.body.variablesReference, { '[0]': i * 10, '[1]': i * 10, '[2]': i * 10, '[3]': i * 10 });

                    log(`${i}: evaluate as array`);
                    let response3 = await ds.evaluateRequest({ expression: 'array_struct_p,[5]', frameId: frameId, context: 'watch' });

                    log(`${i}: compareVariables`);
                    await ds.compareVariables(response3.body.variablesReference, {
                        '[0]': { a: i * 2, b: "'a'", c: 0 },
                        '[2]': { a: i * 2 + 2, b: "'c'", c: 2 },
                        '[4]': { a: i * 2 + 4, b: "'e'", c: 4 }
                    });
                }
                await ds.terminate();
            });

            test('conditional breakpoint /se', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine, '/se i == 5');

                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /se',
                    program: debuggee, args: ['vars']
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('conditional breakpoint /py', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine, '/py $i == 5');

                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /py',
                    program: debuggee, args: ['vars']
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('conditional breakpoint /nat', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine, '/nat i == 5');

                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /nat',
                    program: debuggee, args: ['vars']
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('disassembly', async function () {
                //if (triple.endsWith('pc-windows-msvc')) this.skip();
                if (/windows/.test(triple)) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let setBreakpointAsync = ds.setFnBreakpoint('/re disassembly1');
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'disassembly', program: debuggee, args: ['dasm'] });
                let stackTrace = await ds.stackTraceRequest({
                    threadId: stoppedEvent.body.threadId,
                    startFrame: 0, levels: 5
                });
                let sourceRef = stackTrace.body.stackFrames[0].source.sourceReference;
                let source = await ds.sourceRequest({ sourceReference: sourceRef });
                assert.equal(source.body.mimeType, 'text/x-lldb.disassembly');

                // Set a new breakpoint two instructions ahead
                await ds.setBreakpointsRequest({
                    source: { sourceReference: sourceRef },
                    breakpoints: [{ line: 5 }]
                });
                let waitStoppedEvent2 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: stoppedEvent.body.threadId });
                let stoppedEvent2 = await waitStoppedEvent2;
                let stackTrace2 = await ds.stackTraceRequest({
                    threadId: stoppedEvent2.body.threadId,
                    startFrame: 0, levels: 5
                });
                assert.equal(stackTrace2.body.stackFrames[0].source.sourceReference, sourceRef);
                assert.equal(stackTrace2.body.stackFrames[0].line, 5);
                await ds.terminate();
            });

            test('display_html', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(debuggeeSource, '#BP1');
                let setBreakpointAsync = ds.setBreakpoint(debuggeeSource, bpLine, '/py debugger.display_html("<html>", "title", 1) and False');
                let waitForDisplayHtmlAsync = ds.waitForEvent('displayHtml');
                await ds.launch({ name: 'display_html', program: debuggee, args: ["mandelbrot"] });
                await setBreakpointAsync;
                let ev = await waitForDisplayHtmlAsync;
                assert.equal(ev.body.html, "<html>");
                assert.equal(ev.body.title, 'title');
                assert.equal(ev.body.position, 1);
                assert.equal(ev.body.reveal, false);
                await ds.terminate();
            });
        });

        suite('Attach tests', () => {
            // Many Linux systems restrict tracing to parent processes only, which lldb in this case isn't.
            // To allow unrestricted tracing run `echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope`.
            let ptraceLocked = false;
            if (process.platform == 'linux') {
                if (parseInt(fs.readFileSync('/proc/sys/kernel/yama/ptrace_scope', 'ascii')) > 0) {
                    ptraceLocked = true;
                }
            }

            let debuggeeProc: cp.ChildProcess;

            suiteSetup(() => {
                if (ptraceLocked)
                    console.log('ptrace() syscall is locked down: skipping attach tests');
                else
                    debuggeeProc = cp.spawn(debuggee, ['inf_loop'], {});
            })

            suiteTeardown(() => {
                if (debuggeeProc)
                    debuggeeProc.kill()
            })

            test('attach by pid', async function () {
                if (ptraceLocked) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let asyncWaitStopped = ds.waitForEvent('stopped');
                let attachResp = await ds.attach({ name: 'attach by pid', program: debuggee, pid: debuggeeProc.pid, stopOnEntry: true });
                assert.ok(attachResp.success);
                await asyncWaitStopped;
                await ds.terminate();
            });

            test('attach by pid / nostop', async function () {
                if (ptraceLocked) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let stopCount = 0;
                ds.addListener('stopped', () => stopCount += 1);
                ds.addListener('continued', () => stopCount -= 1);
                let attachResp = await ds.attach({ name: 'attach by pid / nostop', program: debuggee, pid: debuggeeProc.pid, stopOnEntry: false });
                assert.ok(attachResp.success);
                assert.ok(stopCount <= 0);
                await ds.terminate();
            });

            test('attach by name', async function () {
                if (ptraceLocked) this.skip();

                let ds = await DebugTestSession.start(adapterLog);
                let asyncWaitStopped = ds.waitForEvent('stopped');
                let attachResp = await ds.attach({ name: 'attach by name', program: debuggee, stopOnEntry: true });
                assert.ok(attachResp.success);
                await asyncWaitStopped;
                await ds.terminate();
            });
        })

        suite('Rust tests', () => {
            test('rust_variables', async function () {
                let ds = await DebugTestSession.start(adapterLog);
                let bpLine = findMarker(rustDebuggeeSource, '#BP1');
                let setBreakpointAsync = ds.setBreakpoint(rustDebuggeeSource, bpLine);
                let waitForStopAsync = ds.waitForStopEvent();
                await ds.launch({ name: 'rust variables', program: rustDebuggee });
                await setBreakpointAsync;
                let stoppedEvent = await waitForStopAsync;
                await ds.verifyLocation(stoppedEvent.body.threadId, rustDebuggeeSource, bpLine);
                let frames = await ds.stackTraceRequest({ threadId: stoppedEvent.body.threadId, startFrame: 0, levels: 1 });
                let scopes = await ds.scopesRequest({ frameId: frames.body.stackFrames[0].id });

                let foo_bar = /windows/.test(triple) ? '"foo\\bar"' : '"foo/bar"';

                let localVars = await ds.readVariables(scopes.body.scopes[0].variablesReference);

                await ds.compareVariables(localVars, {
                    bool_: true,
                    i16_: -16,
                    u16_: 16,
                    i32_: -32,
                    u32_: 32,
                    i64_: -64,
                    u64_: 64,
                    i128_: -128,
                    u128_: 128,
                    isize_: -2,
                    usize_: 2,
                    f32_: 3.1415926535,
                    f64_: 3.1415926535 * 2.0,

                    tuple: '(1, "a", 42)',
                    tuple_ref: '(1, "a", 42)',
                    reg_struct: '{a:1, c:12}',
                    reg_struct_ref: '{a:1, c:12}',
                    array: { '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5 },
                    slice: '(5) &[1, 2, 3, 4, 5]',
                    vec_int: {
                        $: '(10) vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]',
                        '[0]': 1, '[1]': 2, '[9]': 10
                    },
                    vec_str: '(5) vec!["111", "2222", "3333", "4444", "5555", ...]',
                    large_vec: '(20000) vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, ...]',
                    empty_string: '""',
                    string: '"A String"',
                    str_slice: '"String slice"',
                    wstr1: '"Превед йожэг!"',
                    wstr2: '"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!"',
                    cstring: '"C String"',
                    osstring: '"OS String"',
                    path_buf: foo_bar,
                    class: { finally: 1, import: 2, lambda: 3, raise: 4 },
                    boxed: { a: 1, b: '"b"', c: 12 },
                    rc_box: { $: '(refs:1) {...}', a: 1, b: '"b"', c: 12 },
                    rc_box2: { $: '(refs:2) {...}', a: 1, b: '"b"', c: 12 },
                    rc_box2c: { $: '(refs:2) {...}', a: 1, b: '"b"', c: 12 },
                    rc_box3: { $: '(refs:1,weak:1) {...}', a: 1, b: '"b"', c: 12 },
                    rc_weak: { $: '(refs:1,weak:1) {...}', a: 1, b: '"b"', c: 12 },
                    arc_box: { $: '(refs:1,weak:1) {...}', a: 1, b: '"b"', c: 12 },
                    arc_weak: { $: '(refs:1,weak:1) {...}', a: 1, b: '"b"', c: 12 },
                    ref_cell: 10,
                    ref_cell2: '(borrowed:2) 11',
                    ref_cell2_borrow1: 11,
                    ref_cell3: '(borrowed:mut) 12',
                    ref_cell3_borrow: 12,
                });

                if (!triple.endsWith('pc-windows-msvc')) {
                    await ds.compareVariables(localVars, {
                        char_: "'A'",
                        i8_: -8,
                        u8_: 8,
                        unit: '()',

                        reg_enum2: '{0:100, 1:200}',
                        reg_enum3: '{x:11.35, y:20.5}',
                        reg_enum_ref: '{x:11.35, y:20.5}',
                        cstr: '"C String"',
                        osstr: '"OS String"',
                        path: foo_bar,
                        str_tuple: {
                            '0': '"A String"',
                            '1': '"String slice"',
                            '2': '"C String"',
                            '3': '"C String"',
                            '4': '"OS String"',
                            '5': '"OS String"',
                            '6': foo_bar,
                            '7': foo_bar,
                        },
                    });

                    let expected1 = [
                        '("Olaf", 24)',
                        '("Harald", 12)',
                        '("Einar", 25)',
                        '("Conan", 29)',
                    ];
                    let hashValues = await ds.readVariables(localVars['hash'].variablesReference);
                    for (let expectedValue of expected1) {
                        assert.ok(Object.values(hashValues).some(v => v.value == expectedValue), expectedValue);
                    }

                    let expected2 = [
                        '"Olaf"',
                        '"Harald"',
                        '"Einar"',
                        '"Conan"',
                    ];
                    let setValues = await ds.readVariables(localVars['set'].variablesReference);
                    for (let expectedValue of expected2) {
                        assert.ok(Object.values(setValues).some(v => v.value == expectedValue), expectedValue);
                    }

                    if (!triple.endsWith('apple-darwin')) {
                        await ds.compareVariables(localVars, {
                            cstyle_enum1: 'rust_debuggee::CStyleEnum::A',
                            cstyle_enum2: 'rust_debuggee::CStyleEnum::B',
                        });
                    }
                }

                // LLDB does not handle Rust enums well for now
                // reg_enum1: 'A',
                // enc_enum1: 'Some("string")',
                // enc_enum2: 'Nothing',
                // opt_str1: 'Some("string")',
                // opt_str2: 'None',
                // opt_reg_struct1: 'Some({...})',
                // opt_reg_struct2: 'None',
                // tuple_struct: '(3, "xxx", -3)',

                let response1 = await ds.evaluateRequest({
                    expression: 'vec_str', context: 'watch',
                    frameId: frames.body.stackFrames[0].id
                });
                await ds.compareVariables(response1.body.variablesReference, { '[0]': '"111"', '[4]': '"5555"' });

                let response2 = await ds.evaluateRequest({
                    expression: 'string', context: 'watch',
                    frameId: frames.body.stackFrames[0].id
                });
                await ds.compareVariables(response2.body.variablesReference,
                    triple.endsWith('pc-windows-msvc') ?
                        { '[0]': `'A'`, '[7]': `'g'` } :
                        { '[0]': 65, '[7]': 103 }
                );

                // Check format-as-array.
                let response3 = await ds.evaluateRequest({
                    expression: 'array[0],[5]', context: 'watch',
                    frameId: frames.body.stackFrames[0].id
                });
                await ds.compareVariables(response3.body.variablesReference, {
                    '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5,
                });

                await ds.terminate();
            });
        });
    });
}

/////////////////////////////////////////////////////////////////////////////////////////////////

class DebugTestSession extends DebugClient {
    adapter: cp.ChildProcess;
    port: number;

    static async start(logStream: stream.Writable): Promise<DebugTestSession> {
        let session = new DebugTestSession('', '', 'lldb');

        if (process.env.DEBUG_SERVER) {
            session.port = parseInt(process.env.DEBUG_SERVER)
        } else {
            let liblldb = await adapter.findLibLLDB(path.join(extensionRoot, 'lldb'));
            let libpython = await adapter.findLibPython(extensionRoot);
            session.adapter = await adapter.start(liblldb, libpython, {
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

            session.adapter.stdout.pipe(logStream);
            session.adapter.stderr.pipe(logStream);
            session.port = await adapter.getDebugServerPort(session.adapter);
        }

        let logger = (event: dp.Event) => log(`Received event: ${inspect(event, { breakLength: Infinity })}`);
        session.addListener('breakpoint', logger);
        session.addListener('stopped', logger);
        session.addListener('continued', logger);
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
        super.stop();
    }

    async launch(launchArgs: any): Promise<dp.LaunchResponse> {
        launchArgs.terminal = 'console';
        let waitForInit = this.waitForEvent('initialized');
        await this.initializeRequest()
        let launchResp = this.launchRequest(launchArgs);
        await waitForInit;
        this.configurationDoneRequest();
        return launchResp;
    }

    async attach(attachArgs: any): Promise<dp.AttachResponse> {
        let waitForInit = this.waitForEvent('initialized');
        await this.initializeRequest()
        let attachResp = this.attachRequest(attachArgs);
        await waitForInit;
        this.configurationDoneRequest();
        return attachResp;
    }

    async setBreakpoint(file: string, line: number, condition?: string): Promise<dp.SetBreakpointsResponse> {
        await this.waitForEvent('initialized');
        let breakpointResp = await this.setBreakpointsRequest({
            source: { path: file },
            breakpoints: [{ line: line, column: 0, condition: condition }],
        });
        let bp = breakpointResp.body.breakpoints[0];
        log(`Received setBreakpoint response: ${inspect(bp, { breakLength: Infinity })}`);
        // assert.ok(bp.verified);
        // assert.equal(bp.line, line);
        return breakpointResp;
    }

    async setFnBreakpoint(name: string, condition?: string): Promise<dp.SetFunctionBreakpointsResponse> {
        await this.waitForEvent('initialized');
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
        return new Promise<dp.StoppedEvent>(resolve => {
            let handler = (event: dp.StoppedEvent) => {
                if (event.body.reason != 'initial') {
                    session.removeListener('stopped', handler);
                    resolve(event);
                } else {
                    log('Ignored "initial" event');
                }
            };
            session.addListener('stopped', handler);
        });
    }

    async launchAndWaitForStop(launchArgs: any): Promise<dp.StoppedEvent> {
        let waitForStopAsync = this.waitForStopEvent();
        log('launchAndWaitForStop: launching');
        await this.launch(launchArgs);
        log('launchAndWaitForStop: waiting to stop');
        let stoppedEvent = await waitForStopAsync;
        return <dp.StoppedEvent>stoppedEvent;
    }

    async getTopFrameId(threadId: number): Promise<number> {
        let frames = await this.stackTraceRequest({ threadId: threadId, startFrame: 0, levels: 1 });
        return frames.body.stackFrames[0].id;
    }

    async getFrameLocalsRef(frameId: number): Promise<number> {
        let scopes = await this.scopesRequest({ frameId: frameId });
        let localsRef = scopes.body.scopes[0].variablesReference;
        return localsRef;
    }
}

type ValidatorFn = (v: dp.Variable) => boolean;

function findMarker(file: string, marker: string): number {
    let data = fs.readFileSync(file, 'utf8');
    let lines = data.split('\n');
    for (let i = 0; i < lines.length; ++i) {
        let pos = lines[i].indexOf(marker);
        if (pos >= 0) return i + 1;
    }
    throw Error('Marker not found');
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

function log(message: string) {
    testLog.write(`[${timestamp()}] ${message}\n`);
}

function dumpLogs(dest: stream.Writable) {
    dest.write('\n=== Test log ==============\n');
    dest.write(testLog.toString());
    if (testDataLog) {
        dest.write('\n=== Received data log ====\n');
        dest.write(testDataLog.toString());
    }
    dest.write('\n=== Adapter log ===========\n');
    dest.write(adapterLog.toString());
    dest.write('\n===========================\n');
}
