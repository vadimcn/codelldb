'use strict'; 
 
import * as cp from 'child_process'; 
import * as path from 'path'; 
import { Readable, Writable } from 'stream'; 
 
let adapterPath = path.join(__dirname, '..', '..', 'adapter')

let lldb = cp.spawn('lldb', ['-b', '-Q',
            '-O', 'command script import \'' + adapterPath + '\'',
            '-O', 'script adapter.main.run_stdio_session(3,4)'
        ], { 
            stdio: ['ignore', 'ignore', 'ignore', 'pipe', process.stdout],
            cwd: path.join(__dirname, '..', '..')
        }
    ); 
process.stdin.pipe(<Writable>lldb.stdio[3]);
