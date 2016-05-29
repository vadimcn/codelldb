
class Handles:
    def __init__(self):
        self.dict = {}
        self.next_handle = 1000

    def create(self, value):
        h = self.next_handle
        self.dict[h] = value
        self.next_handle += 1
        return h

    def get(self, handle, dflt=None):
        return self.dict.get(handle, dflt)

    def reset(self):
        self.dict.clear()
