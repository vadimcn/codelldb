'use strict';

import * as cp from 'child_process';
import * as path from 'path';
import { format } from 'util';
import { Readable, Writable } from 'stream';

let adapterPath = path.join(__dirname, '..', '..', 'adapter')

var logging = '';
if (process.env.ADAPTER_LOG) {
    logging = format(',log_file=\'%s\', log_level=0',
        new Buffer(process.env.ADAPTER_LOG).toString('base64'));
}

let lldb = cp.spawn('lldb', ['-b', '-Q',
            '-O', format('command script import \'%s\'', adapterPath),
            '-O', format('script adapter.main.run_stdio_session(3,4 %s)', logging)
        ], {
            stdio: ['ignore', 'ignore', 'ignore', 'pipe', process.stdout],
            cwd: path.join(__dirname, '..', '..')
        }
    );
process.stdin.pipe(<Writable>lldb.stdio[3]);
