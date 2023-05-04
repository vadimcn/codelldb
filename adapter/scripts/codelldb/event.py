class Event:
    def __init__(self):
        self._listeners = []

    def add(self, listener):
        self._listeners.append(listener)

    def remove(self, listener):
        self._listeners.remove(listener)

    def emit(self, message):
        for listener in self._listeners:
            listener(message)
