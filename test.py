from gevent.monkey import patch_all
patch_all()

import threading
from typing import Generic
from typing import List
from typing import TypeVar

import envoy_mobile
import gevent
from gevent.pool import Group


def print_response(response: envoy_mobile.Response):
    print(response.status_code)
    print(response.headers)
    print(bytes(response.body))


def print_streamed_error(stream_errored: envoy_mobile.StreamErrored):
    info = stream_errored.args[0]
    print(str(info.envoy_error.error_code))
    print(info.envoy_error.message)
    print(info.envoy_error.attempt_count)


def request_local() -> envoy_mobile.Response:
    channel = GeventChannel()
    envoy_mobile.async_request(
        "GET",
        "https://api.lyft.com/ping",
        None,
        None,
        lambda response: channel.put(response),
    )
    response = channel.get()
    print_response(response)
    return response


T = TypeVar("T")


class GeventChannel(Generic[T]):
    def __init__(self):
        self.hub = gevent.get_hub()
        self.watcher = self.hub.loop.async_()
        self.lock = threading.Lock()
        self.values: List[T] = []

    def put(self, value: T) -> None:
        with self.lock:
            self.values.append(value)
            self.watcher.send()

    def get(self) -> T:
        self.lock.acquire()
        while len(self.values) == 0:
            self.lock.release()
            self.hub.wait(self.watcher)
            self.lock.acquire()

        value: T = self.values.pop(0)
        self.lock.release()
        return value


def main():
    group = Group()
    for _ in range(5):
        group.spawn(request_local)
    group.join()


if __name__ == "__main__":
    main()
