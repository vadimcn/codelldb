class NoFailCommand:
    '''Runs another command ignoring failures.'''

    @staticmethod
    def register(debugger):
        debugger.HandleCommand('command script add -c codelldb.nofail.NoFailCommand nofail')

    def __init__(self, debugger, internal_dict):
        pass

    def __call__(self, debugger, command, exe_ctx, result):
        debugger.HandleCommand(command)
