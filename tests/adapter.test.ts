import { suite, test } from 'mocha';
import * as assert from 'assert';
import * as path from 'path';
import * as cp from 'child_process';
import { initUtils, DebugTestSession, findMarker, log, dumpLogs, logWithStack, char, variablesAsDict, withTimeout } from './testUtils';

const triple = process.env.TARGET_TRIPLE || '';
const buildDir = process.env.BUILD_DIR || path.dirname(__dirname); // tests are located in $buildDir/tests
const sourceDir = process.env.SOURCE_DIR || path.dirname(buildDir); // assume $sourceDir is the parent of $buildDir
const dumpLogsWhen = (process.env.DUMP_LOGS || 'onerror').toLowerCase();

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

generateSuite(triple);

function generateSuite(triple: string) {
    suite(`adapter:${triple}`, () => {

        setup(function () {
            initUtils(buildDir);
        });

        teardown(async function () {
            if (this.currentTest.state == 'failed')
                process.stderr.write(`*** Test FAILED: ${this.currentTest.title}\n${this.currentTest.err.stack}\n`);
            if (dumpLogsWhen != 'never' && (this.currentTest.state == 'failed' || dumpLogsWhen == 'always'))
                dumpLogs(process.stderr);
        });

        suite('Basic', () => {

            test('check python', async function () {
                let ds = await DebugTestSession.start();
                await ds.launch({ name: 'check python', custom: true });
                let result = await ds.evaluateRequest({
                    expression: 'script import lldb; print(lldb.debugger.GetVersionString())',
                    context: '_command'
                });
                assert.ok(result.body.result.startsWith('lldb version'));
                assert.ok(result.body.result.indexOf('rust-enabled') >= 0);

                // Check that LLDB was built with libxml2.
                let result2 = await ds.evaluateRequest({
                    expression: 'script import lldb; s = lldb.SBStream(); lldb.debugger.GetBuildConfiguration().GetAsJSON(s) and None; print(s.GetData())',
                    context: '_command'
                });
                let buildConfig = JSON.parse(result2.body.result);
                assert.ok(buildConfig.xml.value);

                await ds.terminate();
            });

            test('run program to the end', async function () {
                let ds = await DebugTestSession.start();
                let terminatedAsync = ds.waitForEvent('terminated');
                await ds.launch({ name: 'run program to the end', program: debuggee });
                await terminatedAsync;
                await ds.terminate();
            });

            test('run program with modified environment', async function () {
                let ds = await DebugTestSession.start();
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
                let ds = await DebugTestSession.start();
                await ds.launchAndWaitForStop({ name: 'stop on entry', program: debuggee, args: ['inf_loop'], stopOnEntry: true });
                await ds.terminate();
            });

            test('stop on a breakpoint (basic)', async function () {
                let ds = await DebugTestSession.start();
                let waitForExitAsync = ds.waitForEvent('exited');
                let bpLineSource = findMarker(debuggeeSource, '#BP1');
                let stopEvent = await ds.launchAndWaitForStop({ name: 'stop on a breakpoint (basic)', program: debuggee, cwd: path.dirname(debuggee) },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLineSource);
                    });
                await ds.verifyLocation(stopEvent.body.threadId, debuggeeSource, bpLineSource);
                log('Continue');
                await ds.continueRequest({ threadId: 0 });
                log('Wait for exit');
                await waitForExitAsync;
                await ds.terminate();
            });

            test('stop on a breakpoint (same file name)', async function () {
                let ds = await DebugTestSession.start();

                let waitForExitAsync = ds.waitForEvent('exited');

                // let testcase = triple.endsWith('windows-gnu') ?
                //     'header_nodylib' : // FIXME: loading dylib triggers a weird access violation on windows-gnu
                //     'header';
                let testcase = 'header_nodylib';

                let bpLineSource = findMarker(debuggeeSource, '#BP1');
                let bpLineHeader = findMarker(debuggeeHeader, '#BPH1');
                let stopEvent = await ds.launchAndWaitForStop(
                    { name: 'stop on a breakpoint (same file name)', program: debuggee, args: [testcase], cwd: path.dirname(debuggee) },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLineSource);
                        await ds.setBreakpoint(debuggeeHeader, bpLineHeader);
                    });
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

                let ds = await DebugTestSession.start();
                let waitForExitAsync = ds.waitForEvent('exited');

                let bpLineRemote1 = findMarker(debuggeeRemote1, '#BP1');
                let bpLineRemote2 = findMarker(debuggeeRemote2, '#BP1');
                let bpLineRelative = findMarker(debuggeeRelative, '#BP1');
                let bpLineDenorm = findMarker(debuggeeDenorm, '#BP1');

                let sourceMap = null;
                if (process.platform != 'win32') {
                    sourceMap = {
                        '/remote1': path.join(sourceDir, 'debuggee', 'cpp', 'remote1'),
                        '/remote2': path.join(sourceDir, 'debuggee', 'cpp', 'remote2'),
                        '.': path.join(sourceDir, 'debuggee'),
                    };
                } else { // On Windows, LLDB adds current drive letter to drive-less paths.
                    sourceMap = {
                        'C:\\remote1': path.join(sourceDir, 'debuggee', 'cpp', 'remote1'),
                        'C:\\remote2': path.join(sourceDir, 'debuggee', 'cpp', 'remote2'),
                        '.': path.join(sourceDir, 'debuggee'),
                    };
                }
                let stopEvent1 = await ds.launchAndWaitForStop({
                    name: 'stop on a breakpoint (path mapping)', program: debuggee, args: ['weird_path'], cwd: path.dirname(debuggee),
                    sourceMap: sourceMap,
                    relativePathBase: path.join(sourceDir, 'debuggee'),
                    preRunCommands: [
                        'set show target.source-map'
                    ]
                }, async () => {
                    await ds.setBreakpoint(debuggeeRemote1, bpLineRemote1);
                    await ds.setBreakpoint(debuggeeRemote2, bpLineRemote2);
                    await ds.setBreakpoint(debuggeeRelative, bpLineRelative);
                    // await ds.setBreakpoint(debuggeeDenorm, bpLineDenorm);
                });

                await ds.verifyLocation(stopEvent1.body.threadId, debuggeeRemote1, bpLineRemote1);
                await ds.evaluate('break list');

                let waitForStopAsync2 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: 0 });
                logWithStack('Wait for stop 2');
                let stopEvent2 = await waitForStopAsync2;
                await ds.verifyLocation(stopEvent2.body.threadId, debuggeeRemote2, bpLineRemote2);

                let waitForStopAsync3 = ds.waitForStopEvent();
                await ds.continueRequest({ threadId: 0 });
                logWithStack('Wait for stop 3');
                let stopEvent3 = await waitForStopAsync3;
                await ds.verifyLocation(stopEvent3.body.threadId, debuggeeRelative, bpLineRelative);

                // let waitForStopAsync4 = ds.waitForStopEvent();
                // await ds.continueRequest({ threadId: 0 });
                // logWithStack('Wait for stop 4');
                // let stopEvent4 = await waitForStopAsync4;
                // await ds.verifyLocation(stopEvent4.body.threadId, debuggeeDenorm, bpLineDenorm);

                await ds.continueRequest({ threadId: 0 });
                logWithStack('Wait for exit');
                await waitForExitAsync;
                await ds.terminate();
            });

            test('page stack', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP2');
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'page stack', program: debuggee, args: ['deepstack'] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine);
                    });
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

                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'variables', program: debuggee, args: ['vars'] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine);
                    });
                await ds.verifyLocation(stoppedEvent.body.threadId, debuggeeSource, bpLine);
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);

                let locals = variablesAsDict(await ds.readVariables(localsRef));
                await ds.compareVariables(locals, {
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
                    s_ref: { a: 1, b: "'a'", c: 3 },
                    s_ptr_ptr: v => v.value.startsWith('{0x'),

                    s2: { a: 10, b: "'b'", c: 999 },
                    cstr: '"The quick brown fox"',
                    wcstr: 'L"The quick brown fox"',
                    str1: '"The quick brown fox"',
                    str_ptr: '"The quick brown fox"',
                    //str_ref: '"The quick brown fox"',  broken in LLDB 13
                    empty_str: '""',
                    wstr1: 'L"Превед йожэг!"',
                    wstr2: 'L"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!"',

                    invalid_utf8: '"ABC\\xff\\U00000001\\xfeXYZ"',

                    null_s_ptr: '<null>',
                    null_s_ptr_ptr: v => v.value.startsWith('{0x'),
                    invalid_s_ptr: '<invalid address>',
                    void_ptr: v => v.value.startsWith('0x'),
                });

                let fields = await ds.readVariables(locals['anon_union'].variablesReference);
                assert.equal(fields[0].name, '')
                ds.compareVariables(fields[0].variablesReference, { x: 4, w: 4 });
                assert.equal(fields[1].name, '')
                ds.compareVariables(fields[1].variablesReference, { y: 5, h: 5 });

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

            test('variables update', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip();

                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP4');
                let stopAsync = ds.launchAndWaitForStop({ name: 'variables update', program: debuggee, args: ['vars_update'] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine);
                    });
                let vectorExpect: { [key: string]: number; } = {};
                for (let i = 0; i < 10; ++i) {
                    vectorExpect[`[${i}]`] = i;

                    let stoppedEvent = await stopAsync;
                    await ds.verifyLocation(stoppedEvent.body.threadId, debuggeeSource, bpLine);
                    let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                    let localsRef = await ds.getFrameLocalsRef(frameId);
                    await ds.compareVariables(localsRef, { i: i, vector: vectorExpect });
                    stopAsync = ds.waitForStopEvent();
                    await ds.continueRequest({ threadId: 0 });
                }
                await ds.terminate();
            })

            test('expressions', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip();

                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'expressions', program: debuggee, args: ['vars'] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine);
                    });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);

                log('Waiting a+b');
                let response1 = await ds.evaluateRequest({ expression: 'a+b', frameId: frameId, context: 'watch' });
                assert.equal(response1.body.result, '70');

                log('Waiting /py...');
                let response2 = await ds.evaluateRequest({ expression: '/py sum([int(x) for x in $array_int])', frameId: frameId, context: 'watch' });
                assert.equal(response2.body.result, '55'); // sum(1..10)

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
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /se',
                    program: debuggee, args: ['vars']
                }, async () => {
                    await ds.setBreakpoint(debuggeeSource, bpLine, '/se i == 5');
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('conditional breakpoint /py', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /py',
                    program: debuggee, args: ['vars']
                }, async () => {
                    await ds.setBreakpoint(debuggeeSource, bpLine, '/py $i == 5');
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                let vars = await ds.readVariables(localsRef);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('conditional breakpoint /nat', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({
                    name: 'conditional breakpoint /nat',
                    program: debuggee, args: ['vars']
                }, async () => {
                    await ds.setBreakpoint(debuggeeSource, bpLine, '/nat i == 5');
                });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);
                let localsRef = await ds.getFrameLocalsRef(frameId);
                await ds.compareVariables(localsRef, { i: 5 });
                await ds.terminate();
            });

            test('disassembly', async function () {
                if (triple.endsWith('pc-windows-msvc')) this.skip(); // With MSVC, we can't suppress debug info per-file.

                let ds = await DebugTestSession.start();
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'disassembly', program: debuggee, args: ['dasm'] },
                    async () => {
                        await ds.setFnBreakpoint('/re disassembly1');
                    });
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

            test('debugger api', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP3');
                let stoppedEvent = await ds.launchAndWaitForStop({ name: 'expressions', program: debuggee, args: ['vars'] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine);
                    });
                let frameId = await ds.getTopFrameId(stoppedEvent.body.threadId);

                let response1 = await ds.evaluateRequest({
                    expression: '/py type(debugger.evaluate("s1"))', frameId: frameId, context: 'watch'
                });
                assert.ok(response1.body.result.includes('value.Value'), `Actual: ${response1.body.result} `);

                let response2 = await ds.evaluateRequest({
                    expression: '/py type(debugger.evaluate("s1", unwrap=True))', frameId: frameId, context: 'watch'
                });
                assert.ok(response2.body.result.includes('lldb.SBValue'), `Actual: ${response2.body.result} `);

                let response3 = await ds.evaluateRequest({
                    expression: '/py type(debugger.wrap(debugger.evaluate("s1", unwrap=True)))', frameId: frameId, context: 'watch'
                });
                assert.ok(response3.body.result.includes('value.Value'), `Actual: ${response3.body.result} `);

                await ds.terminate();
            });

            test('display_html', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(debuggeeSource, '#BP1');
                let waitForDisplayHtmlAsync = ds.waitForEvent('displayHtml');
                await ds.launchAndWaitForStop({ name: 'display_html', program: debuggee, args: ["mandelbrot"] },
                    async () => {
                        await ds.setBreakpoint(debuggeeSource, bpLine, '/py debugger.display_html("<html>", "title", 1) or True');
                    });
                let ev = await waitForDisplayHtmlAsync;
                assert.equal(ev.body.html, "<html>");
                assert.equal(ev.body.title, 'title');
                assert.equal(ev.body.position, 1);
                assert.equal(ev.body.reveal, false);
                await ds.terminate();
            });
        });

        suite('Attach tests', () => {
            let debuggeeProc: cp.ChildProcess;

            suiteSetup(() => {
                debuggeeProc = cp.spawn(debuggee, ['inf_loop'], {});
                debuggeeProc.on('error', err => log(`Debuggee error: ${err}`));
            })

            suiteTeardown(async () => {
                let debuggeeExit = new Promise((resolve) => debuggeeProc.on('exit', resolve));
                debuggeeProc.kill();
                await withTimeout(3000, debuggeeExit);
            })

            test('attach by pid', async function () {
                let ds = await DebugTestSession.start();
                let asyncWaitStopped = ds.waitForEvent('stopped');
                let attachResp = await ds.attach({ name: 'attach by pid', program: debuggee, pid: debuggeeProc.pid, stopOnEntry: true });
                assert.ok(attachResp.success);
                await asyncWaitStopped;
                await ds.terminate();
            });

            test('attach by pid / nostop', async function () {
                let ds = await DebugTestSession.start();
                let stopCount = 0;
                ds.addListener('stopped', () => stopCount += 1);
                ds.addListener('continued', () => stopCount -= 1);
                let attachResp = await ds.attach({ name: 'attach by pid / nostop', program: debuggee, pid: debuggeeProc.pid, stopOnEntry: false });
                assert.ok(attachResp.success);
                assert.ok(stopCount <= 0);
                await ds.terminate();
            });

            test('attach by path', async function () {
                let ds = await DebugTestSession.start();
                let asyncWaitStopped = ds.waitForEvent('stopped');
                let attachResp = await ds.attach({ name: 'attach by name', program: debuggee, stopOnEntry: true });
                assert.ok(attachResp.success);
                await asyncWaitStopped;
                await ds.terminate();
            });

            test('attach by name', async function () {
                let ds = await DebugTestSession.start();
                let asyncWaitStopped = ds.waitForEvent('stopped');
                let program = process.platform != 'win32' ? 'debuggee' : 'debuggee.exe';
                let attachResp = await ds.attach({ name: 'attach by name', program: program, stopOnEntry: true });
                assert.ok(attachResp.success);
                await asyncWaitStopped;
                await ds.terminate();
            });
        })

        suite('Rust tests', () => {
            test('rust primitives', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_primitives');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust primitives', program: rustDebuggee }, rustDebuggeeSource, bpLine);
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
                })
                if (!triple.endsWith('pc-windows-msvc')) {
                    await ds.compareVariables(localVars, {
                        char_: "'A'",
                        i8_: -8,
                        u8_: 8,
                        unit: '()',
                    })
                }
                await ds.terminate();
            })

            test('rust enums', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_enums');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust enums', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                if (!triple.endsWith('pc-windows-msvc')) {
                    await ds.compareVariables(localVars, {
                        reg_enum1: {},
                        reg_enum2: { $: '{0:100, 1:200}', 0: 100, 1: 200 },
                        reg_enum3: { $: '{x:11.35, y:20.5}', x: 11.35, y: 20.5 },
                        reg_enum_ref: '{x:11.35, y:20.5}',
                        cstyle_enum1: 'rust_debuggee::CStyleEnum::A',
                        cstyle_enum2: 'rust_debuggee::CStyleEnum::B',
                        enc_enum1: { 0: '"string"' },
                        enc_enum2: {},
                        opt_str1: 'Some("string")',
                        opt_str2: 'None',
                        result_ok: { $: 'Ok("ok")', 0: '"ok"' },
                        result_err: { $: 'Err("err")', 0: '"err"' },
                        cow1: 'Borrowed("their cow")',
                        cow2: 'Owned("my cow")',
                        opt_reg_struct1: { 0: { a: 1, c: 12 } },
                        opt_reg_struct2: 'None',
                    });
                } else {
                    await ds.compareVariables(localVars, {
                        reg_enum1: 'A',
                        reg_enum2: { $: 'B(100, 200)', 0: 100, 1: 200 },
                        reg_enum3: { x: 11.35, y: 20.5 },
                        reg_enum_ref: { x: 11.35, y: 20.5 },
                        cstyle_enum1: 'A',
                        cstyle_enum2: 'B',
                        enc_enum1: { $: 'Some("string")', 0: '"string"' },
                        enc_enum2: 'Nothing',
                        opt_str1: 'Some("string")',
                        opt_str2: 'None',
                        result_ok: { $: 'Ok("ok")', 0: '"ok"' },
                        result_err: { $: 'Err("err")', 0: '"err"' },
                        cow1: 'Borrowed("their cow")',
                        cow2: 'Owned("my cow")',
                        opt_reg_struct1: { $: 'Some({...})', 0: { a: 1, c: 12 } },
                        opt_reg_struct2: 'None',
                    });
                }
                await ds.terminate();
            })

            test('rust structs', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_structs');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust structs', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                await ds.compareVariables(localVars, {
                    tuple: '(1, "a", 42)',
                    tuple_ref: '(1, "a", 42)',
                    reg_struct: '{a:1, c:12}',
                    reg_struct_ref: '{a:1, c:12}',
                    //tuple_struct: '(3, "xxx", -3)',
                })
                await ds.terminate();
            })

            test('rust arrays', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_arrays');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust arrays', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                await ds.compareVariables(localVars, {
                    array: { '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5 },
                    slice: '(5) &[1, 2, 3, 4, 5]',
                    mut_slice: '(5) &[1000, 2000, 3000, 4000, 5000]',
                    vec_int: {
                        $: '(10) vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]',
                        '[0]': 1, '[1]': 2, '[9]': 10
                    },
                    vecdeque_int: {
                        $: '(10) VecDeque[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]',
                        '[0]': 1, '[1]': 2, '[9]': 10
                    },
                    vecdeque_popped: {
                        $: '(9) VecDeque[2, 3, 4, 5, 6, 7, 8, 9, 10]',
                        '[0]': 2, '[1]': 3, '[8]': 10
                    },
                    large_vec: '(20000) vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, ...]',
                    vec_str: {
                        $: '(5) vec!["111", "2222", "3333", "4444", "5555", ...]',
                        '[0]': '"111"', '[4]': '"5555"'
                    },
                    vec_tuple: {
                        '[0]': { 0: 1, 1: 2 }, '[1]': { 0: 2, 1: 3 }, '[2]': { 0: 3, 1: 4 }
                    }
                })

                // // Check format-as-array.
                // let response3 = await ds.evaluateRequest({
                //     expression: 'array[0],[5]', context: 'watch',
                //     frameId: frameId
                // });
                // await ds.compareVariables(response3.body.variablesReference, {
                //     '[0]': 1, '[1]': 2, '[2]': 3, '[3]': 4, '[4]': 5,
                // });
                await ds.terminate();
            })

            test('rust strings', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_strings');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust strings', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                let foo_bar = /windows/.test(triple) ? '"foo\\bar"' : '"foo/bar"';
                await ds.compareVariables(localVars, {
                    empty_string: '""',
                    string: {
                        $: '"A String"',
                        '[0]': char('A'), '[7]': char('g')
                    },
                    str_slice: '"String slice"',
                    wstr1: '"Превед йожэг!"',
                    wstr2: '"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!"',
                    cstring: '"C String"',
                    osstring: '"OS String"',
                    path_buf: foo_bar,
                })
                if (!triple.endsWith('pc-windows-msvc')) {
                    await ds.compareVariables(localVars, {
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
                    })
                }
                await ds.terminate();
            })

            test('rust boxes', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_boxes');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust boxes', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                await ds.compareVariables(localVars, {
                    boxed: { $: '"boxed"' },
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
                })
                await ds.terminate();
            })

            test('rust hashes', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_hashes');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust hashes', program: rustDebuggee }, rustDebuggeeSource, bpLine);
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
                await ds.terminate();
            })

            test('rust misc', async function () {
                let ds = await DebugTestSession.start();
                let bpLine = findMarker(rustDebuggeeSource, '#BP_misc');
                let localVars = await ds.launchStopAndGetVars({ name: 'rust misc', program: rustDebuggee }, rustDebuggeeSource, bpLine);
                await ds.compareVariables(localVars, {
                    class: { finally: 1, import: 2, lambda: 3, raise: 4 },
                })
                await ds.terminate();
            })
        });
    });
}
