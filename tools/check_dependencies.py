#! /usr/bin/env python3

import sys
print('version:', sys.version)
print('executable:', sys.executable)
print('path:', sys.path)

import os
import re
import subprocess

def check(libraries):
    disallowed = False
    allowed_regex = re.compile(sys.argv[2], re.IGNORECASE if sys.platform.startswith('win') else 0)
    for library in libraries:
        if not allowed_regex.fullmatch(library):
            print('{} is not on allowed list'.format(library))
            disallowed = True
    if disallowed:
        sys.exit(1)

def main():
    if sys.platform.startswith('linux'):
        output = subprocess.check_output(['ldd', sys.argv[1]]).decode('utf8')
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(output)]
        check(libraries)
    elif sys.platform.startswith('darwin'):
        output = subprocess.check_output(['otool', '-L', sys.argv[1]]).decode('utf8')
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(output)]
        libraries = [os.path.basename(f) for f in libraries]
        check(libraries)
    elif sys.platform.startswith('win'):
        output = subprocess.check_output(['dumpbin', '/dependents', sys.argv[1]]).decode('utf8')
        regex = re.compile(r'Image has the following dependencies:\s*\r\n\r\n((.+\r\n)*)\s*\r\n')
        match = regex.search(output)
        if not match:
            print('Could not parse dumpbin output:', output)
            sys.exit(1)
        regex = re.compile(r'^\s+([^\s]+)', re.MULTILINE)
        libraries = [match.group(1) for match in regex.finditer(match.group(1))]
        libraries = [os.path.basename(f) for f in libraries]
        check(libraries)
    else:
        print('Unsupported platform')
        sys.exit(1)

if __name__ == '__main__':
    main()
