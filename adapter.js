var cp = require('child_process');
var process = require('process');

var lldb = cp.spawn('lldb', ['-b', '-O', 'script import adapter; adapter.run_stdio_session()'], {
    stdio: ['pipe', 'pipe', 'ignore'],
    cwd: __dirname
});
process.stdin.pipe(lldb.stdio[0]);
lldb.stdio[1].pipe(process.stdout);

// When lldb exits, we exit too
lldb.on('exit', (code) => {
    process.exit(code);
});