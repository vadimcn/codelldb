from .debug_info import DebugInfoCommand
from .nofail import NoFailCommand

def register(debugger):
    debugger.HandleCommand('script import codelldb')
    debugger.HandleCommand('command script add -c codelldb.commands.DebugInfoCommand debug_info')
    debugger.HandleCommand('command script add -c codelldb.commands.NoFailCommand nofail')
