from __future__ import annotations

from gevent import monkey
monkey.patch_all()

import threading
from typing import Any
from typing import Generic
from typing import List
from typing import Tuple
from typing import TypeVar

import envoy_mobile
import gevent
import pytest
import requests
from gevent.pool import Group


def measure_impl(impl: Any, args: Tuple[Any, Any, Any, Any], concurrent_requests: int):
    group = Group()
    for _ in range(concurrent_requests):
        group.spawn(
            impl,
            *args,
        )
    group.join()


def request_requests(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
) -> requests.Response:
    return requests.request(method, url, data=body, headers=headers)


session = requests.Session()


def request_requests_session(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
):
    return session.request(method, url, data=body, headers=headers)


def request_envoy_mobile(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
) -> envoy_mobile.Response:
    channel = GeventChannel()
    envoy_mobile.async_request(
        method,
        url,
        body,
        headers,
        lambda response: channel.put(response),
    )
    return channel.get()


@pytest.mark.parametrize(
    "impl",
    [
        pytest.param(request_requests, id="requests"),
        pytest.param(request_requests_session, id="requests_session"),
        pytest.param(request_envoy_mobile, id="envoy_requests"),
    ],
)
@pytest.mark.parametrize("concurrent_requests", [1, 10, 100])
def test_performance(benchmark, impl, concurrent_requests):
    benchmark(
        measure_impl,
        impl,
        ("GET", "http://127.0.0.1:8080", None, None),
        # ("GET", "https://api.lyft.com/ping", None, None),
        concurrent_requests,
    )


T = TypeVar("T")


class GeventChannel(Generic[T]):
    def __init__(self):
        self.hub = gevent.get_hub()
        self.watcher = self.hub.loop.async_()
        self.values: List[T] = []

    def put(self, value: T) -> None:
        self.hub.loop.run_callback_threadsafe(self._put_impl, value)

    def _put_impl(self, value: T) -> None:
            self.values.append(value)
            self.watcher.send()

    def get(self) -> T:
        while len(self.values) == 0:
            self.hub.wait(self.watcher)
        return self.values.pop(0)
