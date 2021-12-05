from __future__ import annotations

from .channel import GeventChannel
from .envoy_requests import async_request


def request(
    method: str,
    url: str,
    body: str | bytes | None = None,
    headers: dict[str, str] | None = None,
):
    channel = GeventChannel()
    async_request(
        method,
        url,
        body,
        headers,
        lambda response: channel.put(response)
    )
    return channel.get()


def get(*args, **kwargs):
    return request("GET", *args, **kwargs)


def head(*args, **kwargs):
    return request("HEAD", *args, **kwargs)


def post(*args, **kwargs):
    return request("POST", *args, **kwargs)


def put(*args, **kwargs):
    return request("PUT", *args, **kwargs)


def delete(*args, **kwargs):
    return request("DELETE", *args, **kwargs)


def connect(*args, **kwargs):
    return request("CONNECT", *args, **kwargs)


def options(*args, **kwargs):
    return request("OPTIONS", *args, **kwargs)


def trace(*args, **kwargs):
    return request("TRACE", *args, **kwargs)


def patch(*args, **kwargs):
    return request("PATCH", *args, **kwargs)
