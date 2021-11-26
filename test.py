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


def request_local():
    try:
        response = envoy_mobile.request("GET", "http://127.0.0.1:8080", None, None)
        print_response(response)
    except envoy_mobile.StreamErrored as e:
        print_streamed_error(e)


def main():
    group = Group()
    for _ in range(5):
        group.spawn(request_local)
    group.join()


if __name__ == "__main__":
    main()
