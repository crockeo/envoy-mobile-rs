from typing import Generic
from typing import Optional
from typing import TypeVar

import gevent


T = TypeVar("T")


class GeventChannel(Generic[T]):
    def __init__(self):
        self.consumed = False
        self.hub = gevent.get_hub()
        self.watcher = self.hub.loop.async_()
        self.value: Optional[T] = None

    def put(self, value: T) -> None:
        self.hub.loop.run_callback_threadsafe(self._put_impl, value)

    def _put_impl(self, value: T) -> None:
        if self.value is not None:
            raise Exception("attempting to re-put value")

        self.value = value
        self.watcher.send()

    def get(self) -> T:
        if self.consumed:
            raise Exception("attempting to re-get value")
        while self.value is None:
            self.hub.wait(self.watcher)
        self.consumed = True
        return self.value
