import logging

log = logging.getLogger('handles')

class Handles:
    def __init__(self):
        self.obj_by_handle = {}
        self.next_handle = 1000

    # Stores value, returns an integer handle.
    def create(self, value):
        h = self.next_handle
        self.obj_by_handle[h] = value
        self.next_handle += 1
        return h

    # Lookup a value by handle.
    def get(self, handle, dflt=None):
        return self.obj_by_handle.get(handle, dflt)

    def reset(self):
        self.obj_by_handle.clear()

# A version of Handles that maintains parent-child relationship between stored objects.
# HandleTree tries to preserve the numeric values of handles across calls to reset().
class HandleTree:
    def __init__(self):
        self.obj_by_handle = {}
        self.prev_handle_by_vpath = {}
        self.handle_by_vpath = {}
        self.next_handle = 1000

    # Stores value as a child of the value identified by parent_handle, returns an integer handle.
    def create(self, value, key, parent_handle):
        parent_info = self.obj_by_handle.get(parent_handle)
        if parent_info is not None:
            vpath = parent_info[1] + (key,)
        else:
            vpath = (key,)
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

    # value by handle.
    def get(self, handle, dflt=None):
        pair = self.obj_by_handle.get(handle)
        if pair is not None:
            return pair[0]
        else:
            return dflt

    # (value, vpath) by handle.
    def get_vpath(self, handle, dflt=None):
        pair = self.obj_by_handle.get(handle)
        if pair is not None:
            return pair
        else:
            return dflt

    def reset(self):
        self.obj_by_handle.clear()
        t = self.prev_handle_by_vpath
        self.prev_handle_by_vpath = self.handle_by_vpath
        self.handle_by_vpath = t
        self.handle_by_vpath.clear()

