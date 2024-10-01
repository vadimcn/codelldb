from typing import Callable, Any


class Event:
    def __init__(self):
        self._listeners = []

    def add(self, listener: Callable[[Any], None]):
        '''Add an event listener.'''
        self._listeners.append(listener)

    def remove(self, listener: Callable[[Any], None]):
        '''Remove an event listener.'''
        self._listeners.remove(listener)

    def emit(self, message: Any):
        '''Notify all listeners.'''
        for listener in self._listeners:
            listener(message)
