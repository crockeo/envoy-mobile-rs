from gevent.monkey import patch_all
patch_all()

import envoy_mobile

from benchmark import GeventChannel


def print_response(response: envoy_mobile.Response):
    print(response.status_code)
    print(response.headers)
    print(bytes(response.body))


def print_streamed_error(stream_errored: envoy_mobile.StreamErrored):
    info = stream_errored.args[0]
    print(str(info.envoy_error.error_code))
    print(info.envoy_error.message)
    print(info.envoy_error.attempt_count)


def request_local_sync() -> envoy_mobile.Response:
    return envoy_mobile.request(
        "GET",
        "http://localhost:8080",
        None,
        None,
    )


def request_local() -> envoy_mobile.Response:
    channel = GeventChannel()
    envoy_mobile.async_request(
        "GET",
        "http://localhost:8080",
        None,
        None,
        lambda response: channel.put_threadsafe(response),
    )
    return channel.get()


def main():
    while True:
        response = request_local_sync()
        print_response(response)


if __name__ == "__main__":
    main()
