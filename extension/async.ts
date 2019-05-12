import * as fs from 'fs';
import * as cp from 'child_process';
import { promisify } from 'util';

export const readdirAsync = promisify(fs.readdir);
export const readFileAsync = promisify(fs.readFile);
export const existsAsync = promisify(fs.exists);
export const statAsync = promisify(fs.stat);
export const execFileAsync = promisify(cp.execFile);
