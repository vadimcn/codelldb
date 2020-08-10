#! /usr/bin/env python3

import sys
import os
import re
import subprocess
import argparse

def check_dependencies(libraries, wl_regex):
    clean = True
    for library in libraries:
        if not wl_regex.fullmatch(library):
            print('  {} is not in the whitelist'.format(library))
            clean = False
    return clean

def get_dependencies(binary):
    if sys.platform.startswith('linux'):
        output = subprocess.check_output(['ldd', binary]).decode('utf8')
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(output)]
        return libraries
    elif sys.platform.startswith('darwin'):
        output = subprocess.check_output(['otool', '-L', binary]).decode('utf8')
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(output)]
        libraries = [os.path.basename(f) for f in libraries]
        return libraries
    elif sys.platform.startswith('win'):
        output = subprocess.check_output(['dumpbin', '/dependents', binary]).decode('utf8')
        regex = re.compile(r'Image has the following dependencies:\s*\r\n\r\n((.+\r\n)*)\s*\r\n')
        match = regex.search(output)
        if not match:
            print('Could not parse dumpbin output:', output)
            sys.exit(1)
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(match.group(1))]
        libraries = [os.path.basename(f) for f in libraries]
        return libraries
    else:
        print('Unsupported platform')
        sys.exit(1)

def check_file(fpath, wl_regex):
    if sys.platform.startswith('win'):
        if not (fpath.endswith('.exe') or fpath.endswith('.dll')):
            return True
    else:
        if not ('.so' in fpath or '.dylib' in fpath or (not fpath.endswith('.py') and os.access(fpath, os.X_OK))):
            return True

    print('Checking', fpath)
    deps = get_dependencies(fpath)
    return check_dependencies(deps, wl_regex)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory')
    parser.add_argument('whitelist')
    args = parser.parse_args()

    wl_regex = re.compile(args.whitelist, re.IGNORECASE if sys.platform.startswith('win') else 0)

    clean = True
    for entry in os.scandir(args.directory):
        if not entry.is_dir():
            clean = check_file(entry.path, wl_regex) and clean

    if not clean:
        sys.exit(1)

if __name__ == '__main__':
    main()
