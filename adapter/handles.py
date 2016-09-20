import logging

log = logging.getLogger('handles')

class Handles:
    def __init__(self):
        self.obj_by_handle = {}
        self.next_handle = 1000

    def create(self, value):
        h = self.next_handle
        self.obj_by_handle[h] = value
        self.next_handle += 1
        return h

    def get(self, handle, dflt=None):
        return self.obj_by_handle.get(handle, dflt)

    def reset(self):
        self.obj_by_handle.clear()

# A version of Handles that keeps the numerical values of handles stable across reset() calls.
class StableHandles:
    def __init__(self):
        self.obj_by_handle = {}
        self.prev_handle_by_vpath = {}
        self.handle_by_vpath = {}
        self.next_handle = 1000

    def create(self, value, local_id, parent_handle):
        parent_info = self.obj_by_handle.get(parent_handle)
        if parent_info is not None:
            vpath = parent_info[1] + (local_id,)
        else:
            vpath = (local_id,)
        handle = self.prev_handle_by_vpath.get(vpath)
        if handle is None:
            handle = self._next_handle()
        self.obj_by_handle[handle] = (value, vpath)
        self.handle_by_vpath[vpath] = handle
        return handle

    def _next_handle(self):
        h = self.next_handle
        self.next_handle += 1
        return h

    def get(self, handle, dflt=None):
        info = self.obj_by_handle.get(handle)
        if info is not None:
            return info[0]
        else:
            return dflt

    def reset(self):
        self.obj_by_handle.clear()
        t = self.prev_handle_by_vpath
        self.prev_handle_by_vpath = self.handle_by_vpath
        self.handle_by_vpath = t
        self.handle_by_vpath.clear()

