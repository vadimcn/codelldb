#!/usr/bin/python
from __future__ import print_function
import os
import shutil

def main():
    ext_root = os.path.join(os.environ['HOME'], '.vscode/extensions/vscode-lldb')
    shutil.rmtree(ext_root, ignore_errors=True)
    shutil.copytree('out', os.path.join(ext_root, 'out'))
    shutil.copytree('adapter', os.path.join(ext_root, 'adapter'))
    shutil.copytree('syntaxes', os.path.join(ext_root, 'syntaxes'))
    shutil.copyfile('package.json', os.path.join(ext_root, 'package.json'))
    print('Installed to', ext_root)

main()
