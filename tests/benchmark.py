from __future__ import annotations

from gevent import monkey
monkey.patch_all()

from typing import Any
from typing import Generic
from typing import List
from typing import Tuple
from typing import TypeVar

import envoy_requests
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
    response = requests.request(method, url, data=body, headers=headers)
    assert response.status_code == 200
    return response


session = requests.Session()


def request_requests_session(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
):
    response = session.request(method, url, data=body, headers=headers)
    assert response.status_code == 200
    return response


def request_envoy_requests(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
) -> envoy_mobile.Response:
    response = envoy_requests.request(method, url, body, headers)
    assert response.status == 200
    return response


def request_envoy_requests(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
):
    response = envoy_request(method, url)
    assert response.status_code == 200
    return response


@pytest.mark.parametrize(
    "impl",
    [
        pytest.param(request_requests, id="requests"),
        pytest.param(request_requests_session, id="requests-session"),
        pytest.param(request_envoy_requests, id="envoy-requests"),
    ],
)
@pytest.mark.parametrize("concurrent_requests", [1, 10, 100])
def test_performance(benchmark, impl, concurrent_requests):
    benchmark(
        measure_impl,
        impl,
        # ("GET", "http://127.0.0.1:8080/", None, None),
        ("GET", "https://api.lyft.com/ping", None, None),
        # ("GET", "https://www.google.com/", None, None),
        concurrent_requests,
    )
