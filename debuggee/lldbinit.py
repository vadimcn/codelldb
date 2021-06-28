import lldb

class TestSynthProvider:
    """Synthetic provider that generates 4 children from struct containing 2-item array
    """
    names = ['first', 'second', 'third', 'forth']

    def __init__(self, valobj, dict):
        self.valobj = valobj

    def num_children(self):
        return len(self.names)

    def get_child_index(self, name):
        try:
            return self.names.index(name)
        except:
            return -1

    def get_child_at_index(self, index):
        try:
            value = self.valobj.GetChildMemberWithName('d').GetChildAtIndex(index % 2)
            return self.valobj.CreateValueFromData(self.names[index], value.GetData(), value.GetType())

        except Exception as e:
            print(e)
        return None


    def update(self):
        pass

def __lldb_init_module(debugger: lldb.SBDebugger, internal_dict):
    try:
        # Use type summary with synthetic formatter together
        debugger.GetDefaultCategory().AddTypeSummary(
            lldb.SBTypeNameSpecifier("Synth"),
            lldb.SBTypeSummary.CreateWithSummaryString("test size=${svar%#}", lldb.eTypeOptionHideValue),
        )
        debugger.HandleCommand(
            'type synthetic add Synth -l ' + __name__ + '.TestSynthProvider'
        )
        print("Initialized test LLDB synth provider")

    except Exception as e:
        print("Failed to initialize Initialized test LLDB synth provider: " + str(e))
