from gevent.monkey import patch_all
patch_all()

import json

import envoy_requests

def main():
    while True:
        res = envoy_requests.get("https://api.lyft.com/ping")
        print(res.status)
        print(json.dumps(res.headers, indent=2))
        print(bytes(res.data))


if __name__ == "__main__":
    main()
