import lldb

process_events = {
    lldb.SBProcess.eBroadcastBitInterrupt: "eBroadcastBitInterrupt",
    lldb.SBProcess.eBroadcastBitProfileData: "eBroadcastBitProfileData",
    lldb.SBProcess.eBroadcastBitStateChanged: "eBroadcastBitStateChanged",
    lldb.SBProcess.eBroadcastBitSTDERR: "eBroadcastBitSTDERR",
    lldb.SBProcess.eBroadcastBitSTDOUT: "eBroadcastBitSTDOUT",
}

thread_events = {
    lldb.SBThread.eBroadcastBitStackChanged: "eBroadcastBitStackChanged",
    lldb.SBThread.eBroadcastBitThreadSuspended: "eBroadcastBitThreadSuspended",
    lldb.SBThread.eBroadcastBitThreadResumed: "eBroadcastBitThreadResumed",
    lldb.SBThread.eBroadcastBitSelectedFrameChanged: "eBroadcastBitSelectedFrameChanged",
    lldb.SBThread.eBroadcastBitThreadSelected: "eBroadcastBitThreadSelected",
}

breakpoint_events = {
    lldb.eBreakpointEventTypeThreadChanged: "eBreakpointEventTypeThreadChanged",
    lldb.eBreakpointEventTypeLocationsRemoved: "eBreakpointEventTypeLocationsRemoved",
    lldb.eBreakpointEventTypeInvalidType: "eBreakpointEventTypeInvalidType",
    lldb.eBreakpointEventTypeLocationsAdded: "eBreakpointEventTypeLocationsAdded",
    lldb.eBreakpointEventTypeAdded: "eBreakpointEventTypeAdded",
    lldb.eBreakpointEventTypeRemoved: "eBreakpointEventTypeRemoved",
    lldb.eBreakpointEventTypeLocationsResolved: "eBreakpointEventTypeLocationsResolved",
    lldb.eBreakpointEventTypeEnabled: "eBreakpointEventTypeEnabled",
    lldb.eBreakpointEventTypeDisabled: "eBreakpointEventTypeDisabled",
    lldb.eBreakpointEventTypeCommandChanged: "eBreakpointEventTypeCommandChanged",
    lldb.eBreakpointEventTypeConditionChanged: "eBreakpointEventTypeConditionChanged",
    lldb.eBreakpointEventTypeIgnoreChanged: "eBreakpointEventTypeIgnoreChanged",
}

process_states = {
    lldb.eStateUnloaded: "eStateUnloaded",
    lldb.eStateConnected: "eStateConnected",
    lldb.eStateAttaching: "eStateAttaching",
    lldb.eStateLaunching: "eStateLaunching",
    lldb.eStateStopped: "eStateStopped",
    lldb.eStateCrashed: "eStateCrashed",
    lldb.eStateSuspended: "eStateSuspended",
    lldb.eStateRunning: "eStateRunning",
    lldb.eStateStepping: "eStateStepping",
    lldb.eStateDetached: "eStateDetached",
    lldb.eStateExited: "eStateExited",
}

stop_reasons = {
    lldb.eStopReasonInvalid: "eStopReasonInvalid",
    lldb.eStopReasonNone: "eStopReasonNone",
    lldb.eStopReasonTrace: "eStopReasonTrace",
    lldb.eStopReasonBreakpoint: "eStopReasonBreakpoint",
    lldb.eStopReasonWatchpoint: "eStopReasonWatchpoint",
    lldb.eStopReasonSignal: "eStopReasonSignal",
    lldb.eStopReasonException: "eStopReasonException",
    lldb.eStopReasonPlanComplete: "eStopReasonPlanComplete",
    lldb.eStopReasonThreadExiting: "eStopReasonThreadExiting",
}

def print_event(event):
    if lldb.SBProcess.EventIsProcessEvent(event):
        type = event.GetType()
        if type == lldb.SBProcess.eBroadcastBitStateChanged:
            state = lldb.SBProcess.GetStateFromEvent(event)
            stateStr = ""
            if type == lldb.SBProcess.eBroadcastBitStateChanged:
                stateStr = process_states[state]
            print "@@@ SBProcess Event", process_events[type], stateStr

            if state == lldb.eStateStopped:
                process = lldb.SBProcess.GetProcessFromEvent(event)
                for thread in process:
                    stop_reason = thread.GetStopReason()
                    print "@@@ Thread %d: %s" % (thread.GetThreadID(), stop_reasons[stop_reason])

    elif lldb.SBThread.EventIsProcessEvent(event):
        type = lldb.SBThread.GetThreadEventTypeFromEvent(event);
        print "@@@ SBThread Event", thread_events[type]
    elif lldb.SBBreakpoint.EventIsProcessEvent(event):
        type = lldb.SBBreakpoint.GetBreakpointEventTypeFromEvent(event);
        print "@@@ SBBreakpoint Event", breakpoint_events[type]
    else:
        print "@@@ ??? event"
